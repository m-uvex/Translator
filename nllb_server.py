#!/usr/bin/env python3
"""Lokaler Uebersetzungs-Server fuer NLLB-200.

  python nllb_server.py                    Server starten
  python nllb_server.py --install nllb:600M
  python nllb_server.py --installed        installierte IDs als JSON
  python nllb_server.py --purge            Modell-Caches loeschen

Hinweis: Omnilingual MT (OMT) von Meta ist derzeit (Stand 2026) nicht als
oeffentlich herunterladbares Modell verfuegbar - die Checkpoints wurden
weder auf Hugging Face noch auf GitHub veroeffentlicht (nur Paper und der
BOUQuET-Benchmark). Sobald das der Fall ist, kann der OMT-Block in
FAMILIES wieder aktiviert werden. Client- und Editor-Code sind bereits
darauf vorbereitet (omt:-Praefix, engineOf, isMt, drawMt).
"""
import gc, json, os, re, shutil, sys, threading, time
from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
from pathlib import Path
from urllib.parse import parse_qs, urlparse

HERE = Path(__file__).resolve().parent
PORT = int(os.environ.get("NLLB_PORT", "8765"))

# Die Repo-IDs sind hier zentral gepflegt.
#
# `kind` sagt, wie der Server das Modell bedient: "seq2seq" sind die alten
# NLLB-Modelle (eigener Tokenizer-Bos je Zielsprache, zeilenweise), "chat" sind
# reine Sprachmodelle, die eine Anweisung bekommen (HY-MT, MiLMMT). `langs`
# traegt bei den Chat-Familien die offizielle Sprachliste der Modellkarte; nur
# diese Paare bietet die App dann an. NLLB erzeugt seine Liste weiter aus dem
# Tokenizer (langs: None).
#
# Die Namen in `prompt` sind genau die Schreibweisen aus der Modellkarte. Sie
# landen im Prompt und duerfen nicht uebersetzt werden; `zh` ist der chinesische
# Name fuer die chinesische Vorlage von HY-MT.
def _langs(rows):
    return [{"code": c, "name": n, "prompt": p, **({"zh": z} if z else {})} for c, n, p, z in rows]

# Tencent HY-MT 1.5: 33 Sprachen, dazu fuenf chinesische Minderheiten- und
# Dialektvarianten (bo, kk, mn, ug, yue) - zusammen 38 Codes.
HY_LANGS = _langs([
    ("zh", "Chinese", "Chinese", "\u4e2d\u6587"),
    ("zh-Hant", "Traditional Chinese", "Traditional Chinese", "\u7e41\u4f53\u4e2d\u6587"),
    ("en", "English", "English", "\u82f1\u8bed"),
    ("ja", "Japanese", "Japanese", "\u65e5\u8bed"),
    ("ko", "Korean", "Korean", "\u97e9\u8bed"),
    ("fr", "French", "French", "\u6cd5\u8bed"),
    ("de", "German", "German", "\u5fb7\u8bed"),
    ("es", "Spanish", "Spanish", "\u897f\u73ed\u7259\u8bed"),
    ("pt", "Portuguese", "Portuguese", "\u8461\u8404\u7259\u8bed"),
    ("it", "Italian", "Italian", "\u610f\u5927\u5229\u8bed"),
    ("nl", "Dutch", "Dutch", "\u8377\u5170\u8bed"),
    ("pl", "Polish", "Polish", "\u6ce2\u5170\u8bed"),
    ("cs", "Czech", "Czech", "\u6377\u514b\u8bed"),
    ("ru", "Russian", "Russian", "\u4fc4\u8bed"),
    ("uk", "Ukrainian", "Ukrainian", "\u4e4c\u514b\u5170\u8bed"),
    ("tr", "Turkish", "Turkish", "\u571f\u8033\u5176\u8bed"),
    ("ar", "Arabic", "Arabic", "\u963f\u62c9\u4f2f\u8bed"),
    ("he", "Hebrew", "Hebrew", "\u5e0c\u4f2f\u6765\u8bed"),
    ("fa", "Persian", "Persian", "\u6ce2\u65af\u8bed"),
    ("hi", "Hindi", "Hindi", "\u5370\u5730\u8bed"),
    ("bn", "Bengali", "Bengali", "\u5b5f\u52a0\u62c9\u8bed"),
    ("gu", "Gujarati", "Gujarati", "\u53e4\u5409\u62c9\u7279\u8bed"),
    ("mr", "Marathi", "Marathi", "\u9a6c\u62c9\u5730\u8bed"),
    ("ta", "Tamil", "Tamil", "\u6cf0\u7c73\u5c14\u8bed"),
    ("te", "Telugu", "Telugu", "\u6cf0\u5362\u56fa\u8bed"),
    ("ur", "Urdu", "Urdu", "\u4e4c\u5c14\u90fd\u8bed"),
    ("th", "Thai", "Thai", "\u6cf0\u8bed"),
    ("vi", "Vietnamese", "Vietnamese", "\u8d8a\u5357\u8bed"),
    ("id", "Indonesian", "Indonesian", "\u5370\u5c3c\u8bed"),
    ("ms", "Malay", "Malay", "\u9a6c\u6765\u8bed"),
    ("tl", "Filipino", "Filipino", "\u83f2\u5f8b\u5bbe\u8bed"),
    ("km", "Khmer", "Khmer", "\u9ad8\u68c9\u8bed"),
    ("my", "Burmese", "Burmese", "\u7f05\u7538\u8bed"),
    ("bo", "Tibetan", "Tibetan", "\u85cf\u8bed"),
    ("kk", "Kazakh", "Kazakh", "\u54c8\u8428\u514b\u8bed"),
    ("mn", "Mongolian", "Mongolian", "\u8499\u53e4\u8bed"),
    ("ug", "Uyghur", "Uyghur", "\u7ef4\u543e\u5c14\u8bed"),
    ("yue", "Cantonese", "Cantonese", "\u7ca4\u8bed"),
])

# Xiaomi MiLMMT-46 (v1.0): 46 Sprachen. Die Namen sind die der Modellkarte;
# die Vorlage des Modells verlangt genau diese Schreibweisen ("Tagalog",
# "Chinese (Simplified)" usw.).
MI_LANGS = _langs([(c, n, n, "") for c, n in [
    ("ar", "Arabic"), ("az", "Azerbaijani"), ("bg", "Bulgarian"), ("bn", "Bengali"),
    ("ca", "Catalan"), ("cs", "Czech"), ("da", "Danish"), ("de", "German"),
    ("el", "Greek"), ("en", "English"), ("es", "Spanish"), ("fa", "Persian"),
    ("fi", "Finnish"), ("fr", "French"), ("he", "Hebrew"), ("hi", "Hindi"),
    ("hr", "Croatian"), ("hu", "Hungarian"), ("id", "Indonesian"), ("it", "Italian"),
    ("ja", "Japanese"), ("kk", "Kazakh"), ("km", "Khmer"), ("ko", "Korean"),
    ("lo", "Lao"), ("ms", "Malay"), ("my", "Burmese"), ("nl", "Dutch"),
    ("no", "Norwegian"), ("pl", "Polish"), ("pt", "Portuguese"), ("ro", "Romanian"),
    ("ru", "Russian"), ("sk", "Slovak"), ("sl", "Slovenian"), ("sv", "Swedish"),
    ("ta", "Tamil"), ("th", "Thai"), ("tl", "Tagalog"), ("tr", "Turkish"),
    ("ur", "Urdu"), ("uz", "Uzbek"), ("vi", "Vietnamese"), ("yue", "Cantonese"),
    ("zh", "Chinese (Simplified)"), ("zh-Hant", "Chinese (Traditional)"),
]])

FAMILIES = {
    "nllb": {"label": "NLLB-200", "kind": "seq2seq", "langs": None, "models": {
        "600M": {"repo": "facebook/nllb-200-distilled-600M", "gb": 2.5, "dtype": "float32"},
        "1.3B": {"repo": "facebook/nllb-200-distilled-1.3B", "gb": 5.5, "dtype": "float32"},
        "3.3B": {"repo": "facebook/nllb-200-3.3B", "gb": 17.6, "dtype": "bfloat16"},
    }},
    "hy": {"label": "HY-MT", "kind": "chat", "langs": HY_LANGS, "models": {
        "1.8B": {"repo": "tencent/HY-MT1.5-1.8B", "gb": 3.6, "dtype": "bfloat16"},
        "7B": {"repo": "tencent/HY-MT1.5-7B", "gb": 15.2, "dtype": "bfloat16"},
    }},
    "milmmt": {"label": "MiLMMT-46", "kind": "chat", "langs": MI_LANGS, "models": {
        "1B": {"repo": "xiaomi-research/MiLMMT-46-1B-v1.0", "gb": 2.1, "dtype": "bfloat16"},
        "4B": {"repo": "xiaomi-research/MiLMMT-46-4B-v1.0", "gb": 8.6, "dtype": "bfloat16"},
    }},
    # ------------------------------------------------------------------
    # Omnilingual MT (Meta) - derzeit NICHT verfuegbar.
    # Meta hat OMT im Maerz 2026 nur als Paper/Benchmark veroeffentlicht;
    # die Modellgewichte (OMT-LLaMA 1B/3B/8B, OMT-NLLB 3B) sind weder auf
    # Hugging Face noch auf GitHub als Download vorhanden. Ein Versuch,
    # sie zu laden, endet mit "401 Repository Not Found".
    # Sobald Meta die Checkpoints bereitstellt, hier mit korrekten
    # Repo-Namen wieder aktivieren:
    #
    # "omt": {"label": "Omnilingual MT", "models": {
    #     "1B": {"repo": "facebook/<richtiger-name>", "gb": 4.5, "dtype": "float32"},
    #     "3B": {"repo": "facebook/<richtiger-name>", "gb": 12.0, "dtype": "bfloat16"},
    #     "8B": {"repo": "facebook/<richtiger-name>", "gb": 32.0, "dtype": "bfloat16"},
    # }},
    # ------------------------------------------------------------------
}
# Zustandsdateien: normalerweise neben dem Skript, aber NLLB_STATE_DIR hat
# Vorrang. Die Electron-Fassung schickt dort den Benutzerordner hin, weil der
# Programmordner (app.asar.unpacked) bei jedem Update ersetzt wird und die
# Angabe „installiert“ sonst verloren ginge, obwohl die Gewichte im
# Hugging-Face-Cache bleiben. Die Rust-Fassung und die CLI setzen die Variable
# nicht und schreiben weiter neben nllb_server.py.
STATE_DIR = Path(os.environ.get("NLLB_STATE_DIR") or HERE)
try:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
except Exception:
    STATE_DIR = HERE
STATE = STATE_DIR / "nllb-models.json"
LANGF = STATE_DIR / "nllb-languages.json"
CODE = re.compile(r"^[a-z]{3}_[A-Z][a-z]{3}$")
lock = threading.RLock()
jobs = {}
cur = {"id": None, "tok": None, "model": None, "fam": None}
# Laufende Uebersetzungen, die der Client abgebrochen hat. Die IDs kommen mit
# /translate und werden von /cancel gesetzt; die StoppingCriteria in
# translate_line pruefen die Menge waehrend model.generate.
cancelled = set()

def all_models():
    for fam, f in FAMILIES.items():
        for mid, m in f["models"].items():
            yield f"{fam}:{mid}", fam, mid, m

def find(full):
    if not full: return None, None, None
    fam, _, mid = full.partition(":")
    if fam in FAMILIES and mid in FAMILIES[fam]["models"]:
        return fam, mid, FAMILIES[fam]["models"][mid]
    return None, None, None

def cache_dir(repo):
    try:
        from huggingface_hub.constants import HF_HUB_CACHE
        root = Path(HF_HUB_CACHE)
    except Exception:
        root = Path(os.environ.get("HF_HOME", Path.home() / ".cache" / "huggingface")) / "hub"
    return root / ("models--" + repo.replace("/", "--"))

def blobs_size(repo):
    b = cache_dir(repo) / "blobs"
    return sum(f.stat().st_size for f in b.glob("*") if f.is_file()) if b.exists() else 0

def installed():
    try: ids = set(json.loads(STATE.read_text("utf-8")))
    except Exception: ids = set()
    out = set()
    for full, fam, mid, m in all_models():
        if full in ids and cache_dir(m["repo"]).exists(): out.add(full)
    return out

def mark(full, on):
    ids = installed(); (ids.add if on else ids.discard)(full)
    STATE.write_text(json.dumps(sorted(ids)), "utf-8")

def download(full):
    from huggingface_hub import HfApi, snapshot_download
    fam, mid, m = find(full); repo = m["repo"]
    st = any(f.endswith(".safetensors") for f in HfApi().list_repo_files(repo))
    ignore = ["*.msgpack", "*.h5", "*.ot", "*.onnx", "*.bin" if st else "*.safetensors"]
    snapshot_download(repo, ignore_patterns=ignore)
    mark(full, True)

def start_job(full):
    fam, mid, m = find(full)
    if not m: raise RuntimeError(f"Unbekanntes Modell: {full}")
    with lock:
        if jobs.get(full, {}).get("running"): return
        jobs[full] = {"running": True, "error": None}
    def run():
        try: download(full)
        except Exception as e: jobs[full]["error"] = str(e)[:200]
        finally: jobs[full]["running"] = False
    threading.Thread(target=run, daemon=True).start()

def delete(full):
    with lock:
        if jobs.get(full, {}).get("running"): raise RuntimeError("Download laeuft noch")
        fam, mid, m = find(full)
        if not m: raise RuntimeError(f"Unbekanntes Modell: {full}")
        if cur["id"] == full: cur.update(id=None, tok=None, model=None, fam=None); gc.collect()
        shutil.rmtree(cache_dir(m["repo"]), ignore_errors=True)
        mark(full, False)

def model_info():
    inst = installed(); out = []
    for full, fam, mid, m in all_models():
        j = jobs.get(full, {}); run = bool(j.get("running"))
        out.append({"family": fam, "familyLabel": FAMILIES[fam]["label"], "id": mid, "gb": m["gb"], "installed": full in inst and not run,
                    "downloading": run,
                    "progress": min(0.99, blobs_size(m["repo"]) / (m["gb"] * 1e9)) if run else None,
                    "error": j.get("error")})
    return out

def languages():
    if LANGF.exists(): return json.loads(LANGF.read_text("utf-8"))
    from transformers import AutoTokenizer
    import langcodes
    tok = AutoTokenizer.from_pretrained(FAMILIES["nllb"]["models"]["600M"]["repo"])
    langs = []
    for code in sorted(c for c in tok.get_vocab() if CODE.match(c)):
        iso, script = code.split("_")
        try: name = langcodes.Language.get(iso).display_name("en")
        except Exception: name = iso
        langs.append({"code": code, "name": name if script == "Latn" else f"{name} ({script})"})
    langs.sort(key=lambda l: l["name"].lower())
    LANGF.write_text(json.dumps(langs, ensure_ascii=False), "utf-8")
    return langs

def lang_list(fam):
    """Sprachliste einer Familie. NLLB kommt aus dem Tokenizer, die Chat-Familien
    bringen ihre offizielle Liste mit."""
    f = FAMILIES.get(fam)
    if not f: return None
    return languages() if f["kind"] == "seq2seq" else [dict(x) for x in f["langs"]]

def lang_name(fam, code, chinese=False):
    for l in FAMILIES[fam]["langs"] or []:
        if l["code"] == code:
            return (l.get("zh") or l["prompt"]) if chinese else l["prompt"]
    return code

def prompt_for(fam, text, src, tgt, gloss=None, ctxt=None):
    """Prompt nach der Vorlage der jeweiligen Modellkarte.

    HY-MT hat zwei Vorlagen: chinesisch, sobald Chinesisch beteiligt ist, sonst
    englisch. Fuer beide gibt es zusaetzlich eine Terminologie- und eine
    Zusammenhang-Vorlage, die hier mit den Eintraegen des Glossars bzw. dem
    Zusammenhangstext gefuellt werden.

    MiLMMT-46 kennt nur die eine Zeile "Translate this from X to Y"; Glossar und
    Zusammenhang bleiben dort aussen vor, statt sie als Fremdanweisung
    anzuhaengen (die Modellkarte sieht das nicht vor).
    """
    text = "" if text is None else str(text)
    if fam == "hy":
        gl = [g for g in (gloss or []) if str(g.get("s") or "").strip() and str(g.get("t") or "").strip()]
        ctx = (ctxt or "").strip()
        zh = src.startswith("zh") or tgt.startswith("zh")
        tn = lang_name(fam, tgt, zh)
        # Die Terminologie-Vorlage der Modellkarte ist chinesisch formuliert
        # („参考下面的翻译：“ / „X 翻译成 Y“). Sie wird fuer beide Zielsprachen so
        # gesetzt, weil HY-MT nur diese Fassung kennt; die Anweisung darunter
        # bleibt englisch, wenn kein Chinesisch beteiligt ist.
        head = ""
        if gl:
            head = "参考下面的翻译：\n" + "\n".join(f'{str(g["s"]).strip()} 翻译成 {str(g["t"]).strip()}' for g in gl) + "\n\n"
        if ctx:
            if zh:
                return head + f"{ctx}\n参考上面的信息，把下面的文本翻译成{tn}，注意不需要翻译上文，也不要额外解释：\n{text}"
            return head + f"{ctx}\nWith the information above in mind, translate the following segment into {tn}, without additional explanation.\n\n{text}"
        if zh:
            return head + f"将以下文本翻译为{tn}，注意只需要输出翻译后的结果，不要额外解释：\n\n{text}"
        return head + f"Translate the following segment into {tn}, without additional explanation.\n\n{text}"
    sn, tn = lang_name(fam, src), lang_name(fam, tgt)
    return f"Translate this from {sn} to {tn}:\n{sn}: {text}\n{tn}:"

def load_chat(repo, kw):
    # MiLMMT ist ein Gemma-3-Modell mit Bildturm, HY-MT ein reines Sprachmodell.
    # Je nach transformers-Fassung heisst die Klasse anders; der Bildturm bleibt
    # ungenutzt, generate() laeuft allein ueber input_ids.
    import transformers
    err = None
    for name in ("AutoModelForCausalLM", "AutoModelForImageTextToText", "AutoModelForMultimodalLM"):
        cls = getattr(transformers, name, None)
        if not cls: continue
        try: return cls.from_pretrained(repo, local_files_only=True, **kw)
        except Exception as e: err = e
    raise err

def load(full):
    if cur["id"] == full: return
    import torch
    from transformers import AutoTokenizer
    fam, mid, m = find(full)
    if not m: raise RuntimeError(f"Unbekanntes Modell: {full}")
    cur.update(id=None, tok=None, model=None, fam=None); gc.collect()
    repo = m["repo"]; tok = AutoTokenizer.from_pretrained(repo, local_files_only=True)
    kw = {"low_cpu_mem_usage": True}
    if m["dtype"] == "bfloat16": kw["dtype"] = torch.bfloat16
    if FAMILIES[fam]["kind"] == "seq2seq":
        from transformers import AutoModelForSeq2SeqLM
        try: model = AutoModelForSeq2SeqLM.from_pretrained(repo, local_files_only=True, **kw)
        except TypeError:
            if "dtype" in kw: kw["torch_dtype"] = kw.pop("dtype")
            model = AutoModelForSeq2SeqLM.from_pretrained(repo, local_files_only=True, **kw)
    else:
        model = load_chat(repo, kw)
    cur.update(id=full, tok=tok, model=model.eval(), fam=fam)

# Modell aus dem Speicher nehmen. Der Server laeuft weiter, die naechste
# Anfrage laedt es erneut - das dauert beim ersten Mal 20 bis 40 s, gibt aber
# den Arbeitsspeicher frei, wenn die App nur noch Ollama braucht.
def unload():
    with lock:
        had = cur["id"]
        cur.update(id=None, tok=None, model=None, fam=None)
    gc.collect()
    try:
        import torch
        if torch.cuda.is_available(): torch.cuda.empty_cache()
    except Exception:
        pass
    return had

def translate_line(text, src, tgt, req=None, gloss=None, ctxt=None):
    import torch
    from transformers import StoppingCriteria, StoppingCriteriaList
    tok, model = cur["tok"], cur["model"]

    class Cancel(StoppingCriteria):
        def __call__(self, input_ids, scores, **kwargs):
            return bool(req and req in cancelled)

    if FAMILIES[cur["fam"]]["kind"] != "seq2seq":
        p = prompt_for(cur["fam"], text, src, tgt, gloss, ctxt)
        # HY-MT traegt seine Sonderzeichen im Chat-Template; MiLMMT hat nur eine
        # Vorlage, die den Inhalt unveraendert aneinanderhaengt. Fehlt sie, geht
        # der Prompt so wie er ist an den Tokenizer.
        try:
            p = tok.apply_chat_template([{"role": "user", "content": p}], tokenize=False, add_generation_prompt=True)
        except Exception:
            pass
        enc = tok(p, return_tensors="pt", truncation=True, max_length=4096)
        n = int(enc["input_ids"].shape[1])
        pad = tok.pad_token_id if tok.pad_token_id is not None else tok.eos_token_id
        with torch.no_grad():
            out = model.generate(**enc, do_sample=False, num_beams=1, pad_token_id=pad,
                                 max_new_tokens=min(1024, max(64, n * 2)),
                                 stopping_criteria=StoppingCriteriaList([Cancel()]))
        return tok.decode(out[0][n:], skip_special_tokens=True).strip()

    tok.src_lang = src
    enc = tok(text, return_tensors="pt", truncation=True, max_length=512)
    with torch.no_grad():
        out = model.generate(**enc, forced_bos_token_id=tok.convert_tokens_to_ids(tgt), num_beams=4,
                             max_new_tokens=int(enc["input_ids"].shape[1] * 2 + 16),
                             stopping_criteria=StoppingCriteriaList([Cancel()]))
    return tok.batch_decode(out, skip_special_tokens=True)[0]

class Handler(BaseHTTPRequestHandler):
    def _json(self, code, obj):
        body = json.dumps(obj, ensure_ascii=False).encode("utf-8")
        self.send_response(code); self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body))); self._cors(); self.end_headers(); self.wfile.write(body)
    def _cors(self):
        self.send_header("Access-Control-Allow-Origin", "*"); self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
    def do_OPTIONS(self):
        self.send_response(204); self._cors(); self.send_header("Content-Length", "0"); self.end_headers()
    def do_GET(self):
        if self.path.startswith("/languages"):
            q = parse_qs(urlparse(self.path).query)
            L = lang_list((q.get("family") or ["nllb"])[0])
            if L is None: return self._json(404, {"error": "unbekannte Familie"})
            return self._json(200, L)
        elif self.path == "/models": self._json(200, model_info())
        elif self.path == "/health": self._json(200, {"ok": True, "loaded": cur["id"], "port": PORT})
        else: self._json(404, {"error": "not found"})
    def do_POST(self):
        try:
            d = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
            if self.path == "/cancel":
                req = d.get("req")
                if req:
                    cancelled.add(req)
                    # Schutz gegen verwaiste IDs (Client sendet /cancel evtl. nach
                    # dem Ende der Anfrage); die Menge bleibt so klein.
                    if len(cancelled) > 64: cancelled.clear(); cancelled.add(req)
                return self._json(200, {"ok": True})
            if self.path == "/unload":
                return self._json(200, {"ok": True, "was": unload()})
            full = d.get("id") or d.get("model")
            fam, mid, m = find(full)
            if self.path == "/load":
                if full not in installed(): return self._json(400, {"error": "Modell nicht installiert"})
                with lock:
                    load(full)
                return self._json(200, {"ok": True, "loaded": full})
            if self.path == "/translate":
                if not m: return self._json(400, {"error": "unbekanntes Modell"})
                codes = {l["code"] for l in (lang_list(fam) or [])}
                if d.get("src") not in codes or d.get("tgt") not in codes: return self._json(400, {"error": "unbekannte Sprache"})
                if full not in installed(): return self._json(400, {"error": "Modell nicht installiert"})
                req = d.get("req")
                if req and req in cancelled:
                    cancelled.discard(req)
                    return self._json(200, {"translation": "", "cancelled": True})
                with lock:
                    load(full)
                    out = []
                    for l in d.get("text", "").split("\n"):
                        if req and req in cancelled: break
                        out.append(translate_line(l, d["src"], d["tgt"], req, d.get("gloss"), d.get("ctxt")) if l.strip() else "")
                    res = "\n".join(out)
                aborted = bool(req and req in cancelled)
                if req: cancelled.discard(req)
                return self._json(200, {"translation": res, "cancelled": aborted})
            if not m: return self._json(400, {"error": "unbekanntes Modell"})
            if self.path == "/models/install": start_job(full)
            elif self.path == "/models/delete": delete(full)
            else: return self._json(404, {"error": "not found"})
            self._json(200, {"ok": True})
        except (BrokenPipeError, ConnectionResetError):
            # Der Client hat abgebrochen (Stopp-Knopf); die Antwort kann nicht
            # mehr zugestellt werden. Kein Fehler im Server.
            pass
        except Exception as e:
            try: self._json(500, {"error": str(e)})
            except Exception: pass
    def log_message(self, *a): pass

if __name__ == "__main__":
    args = sys.argv[1:]
    if args[:1] == ["--installed"]:
        print(json.dumps(sorted(installed()))); sys.exit(0)
    if args[:1] == ["--purge"]:
        for full, fam, mid, m in all_models(): shutil.rmtree(cache_dir(m["repo"]), ignore_errors=True)
        try: STATE.unlink()
        except FileNotFoundError: pass
        print("ok"); sys.exit(0)
    if args[:1] == ["--install"]:
        languages()
        for full in args[1:]:
            start_job(full); j = jobs[full]
            while j["running"]:
                info = next(x for x in model_info() if f'{x["family"]}:{x["id"]}' == full)
                print(f'{full}: {int((info["progress"] or 0) * 100)} %', flush=True); time.sleep(2)
            if j["error"]: print("Fehler:", j["error"]); sys.exit(1)
            print(f"{full}: fertig", flush=True)
        sys.exit(0)
    LANGS = languages()
    print(f"Uebersetzungs-Server laeuft auf http://127.0.0.1:{PORT}")
    ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
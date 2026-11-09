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

HERE = Path(__file__).resolve().parent
PORT = int(os.environ.get("NLLB_PORT", "8765"))

# Die Repo-IDs sind hier zentral gepflegt.
FAMILIES = {
    "nllb": {"label": "NLLB-200", "models": {
        "600M": {"repo": "facebook/nllb-200-distilled-600M", "gb": 2.5, "dtype": "float32"},
        "1.3B": {"repo": "facebook/nllb-200-distilled-1.3B", "gb": 5.5, "dtype": "float32"},
        "3.3B": {"repo": "facebook/nllb-200-3.3B", "gb": 17.6, "dtype": "bfloat16"},
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
STATE = HERE / "nllb-models.json"
LANGF = HERE / "nllb-languages.json"
CODE = re.compile(r"^[a-z]{3}_[A-Z][a-z]{3}$")
lock = threading.RLock()
jobs = {}
cur = {"id": None, "tok": None, "model": None}

def all_models():
    for fam, f in FAMILIES.items():
        for mid, m in f["models"].items():
            yield f"{fam}:{mid}", fam, mid, m

def find(full):
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
        if cur["id"] == full: cur.update(id=None, tok=None, model=None); gc.collect()
        shutil.rmtree(cache_dir(m["repo"]), ignore_errors=True)
        mark(full, False)

def model_info():
    inst = installed(); out = []
    for full, fam, mid, m in all_models():
        j = jobs.get(full, {}); run = bool(j.get("running"))
        out.append({"family": fam, "id": mid, "gb": m["gb"], "installed": full in inst and not run,
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

def load(full):
    if cur["id"] == full: return
    import torch
    from transformers import AutoTokenizer, AutoModelForSeq2SeqLM
    fam, mid, m = find(full)
    if not m: raise RuntimeError(f"Unbekanntes Modell: {full}")
    cur.update(id=None, tok=None, model=None); gc.collect()
    repo = m["repo"]; tok = AutoTokenizer.from_pretrained(repo, local_files_only=True)
    kw = {"low_cpu_mem_usage": True}
    if m["dtype"] == "bfloat16": kw["dtype"] = torch.bfloat16
    try: model = AutoModelForSeq2SeqLM.from_pretrained(repo, local_files_only=True, **kw)
    except TypeError:
        if "dtype" in kw: kw["torch_dtype"] = kw.pop("dtype")
        model = AutoModelForSeq2SeqLM.from_pretrained(repo, local_files_only=True, **kw)
    cur.update(id=full, tok=tok, model=model.eval())

def translate_line(text, src, tgt):
    import torch
    tok, model = cur["tok"], cur["model"]; tok.src_lang = src
    enc = tok(text, return_tensors="pt", truncation=True, max_length=512)
    with torch.no_grad():
        out = model.generate(**enc, forced_bos_token_id=tok.convert_tokens_to_ids(tgt), num_beams=4,
                             max_new_tokens=int(enc["input_ids"].shape[1] * 2 + 16))
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
        if self.path == "/languages": self._json(200, LANGS)
        elif self.path == "/models": self._json(200, model_info())
        elif self.path == "/health": self._json(200, {"ok": True})
        else: self._json(404, {"error": "not found"})
    def do_POST(self):
        try:
            d = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
            full = d.get("id") or d.get("model")
            fam, mid, m = find(full)
            if self.path == "/load":
                if full not in installed(): return self._json(400, {"error": "Modell nicht installiert"})
                with lock:
                    load(full)
                return self._json(200, {"ok": True, "loaded": full})
            if self.path == "/translate":
                codes = {l["code"] for l in LANGS}
                if d.get("src") not in codes or d.get("tgt") not in codes: return self._json(400, {"error": "unbekannte Sprache"})
                if full not in installed(): return self._json(400, {"error": "Modell nicht installiert"})
                with lock:
                    load(full)
                    res = "\n".join(translate_line(l, d["src"], d["tgt"]) if l.strip() else "" for l in d.get("text", "").split("\n"))
                return self._json(200, {"translation": res})
            if not m: return self._json(400, {"error": "unbekanntes Modell"})
            if self.path == "/models/install": start_job(full)
            elif self.path == "/models/delete": delete(full)
            else: return self._json(404, {"error": "not found"})
            self._json(200, {"ok": True})
        except Exception as e:
            self._json(500, {"error": str(e)})
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
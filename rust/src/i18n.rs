//! Oberflaechentexte in sechs Sprachen, entsprechend der Electron-Fassung.
//!
//! Aufbau wie in der Vorlage: ein Schluessel, sechs Werte. Fehlt ein Eintrag,
//! wird der Schluessel selbst angezeigt (dann sofort sichtbar, statt still auf
//! Englisch zurueckzufallen).

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Lang {
    De,
    En,
    Nl,
    Es,
    Tr,
    Ar,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::De => "de",
            Lang::En => "en",
            Lang::Nl => "nl",
            Lang::Es => "es",
            Lang::Tr => "tr",
            Lang::Ar => "ar",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        match code {
            "de" => Some(Lang::De),
            "en" => Some(Lang::En),
            "nl" => Some(Lang::Nl),
            "es" => Some(Lang::Es),
            "tr" => Some(Lang::Tr),
            "ar" => Some(Lang::Ar),
            _ => None,
        }
    }

    /// Arabisch stellt die Oberflaeche auf rechts-nach-links.
    pub fn rtl(self) -> bool {
        self == Lang::Ar
    }

    pub fn label(self) -> &'static str {
        match self {
            Lang::De => "Deutsch",
            Lang::En => "English",
            Lang::Nl => "Nederlands",
            Lang::Es => "Español",
            Lang::Tr => "Türkçe",
            Lang::Ar => "العربية",
        }
    }

    pub fn auto_from_system() -> Lang {
        let loc = std::env::var("LANG")
            .or_else(|_| std::env::var("LC_ALL"))
            .unwrap_or_default()
            .to_lowercase();
        Lang::from_code(&loc.chars().take(2).collect::<String>()).unwrap_or(Lang::En)
    }

    pub fn resolve(setting: &str) -> Lang {
        if setting == "auto" {
            Lang::auto_from_system()
        } else {
            Lang::from_code(setting).unwrap_or(Lang::En)
        }
    }
}

pub fn t(lang: Lang, key: &str) -> String {
    let (de, en, nl, es, tr, ar) = strings(key);
    match lang {
        Lang::De => de,
        Lang::En => en,
        Lang::Nl => nl,
        Lang::Es => es,
        Lang::Tr => tr,
        Lang::Ar => ar,
    }
    .to_string()
}

type Six = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

#[rustfmt::skip]
fn strings(k: &str) -> Six {
    match k {
        // --- Rahmen ---------------------------------------------------------
        "app"          => ("Übersetzer", "Translator", "Vertaler", "Traductor", "Çevirmen", "المترجم"),
        "win_min"      => ("Minimieren", "Minimize", "Minimaliseren", "Minimizar", "Simge durumuna küçült", "تصغير"),
        "win_max"      => ("Maximieren", "Maximize", "Maximaliseren", "Maximizar", "Ekranı kapla", "تكبير"),
        "win_close"    => ("Schließen", "Close", "Sluiten", "Cerrar", "Kapat", "إغلاق"),
        "nav_tr"       => ("Übersetzer", "Translator", "Vertaler", "Traductor", "Çevirmen", "المترجم"),
        "nav_ed"       => ("Textbearbeitung", "Text editor", "Tekstbewerking", "Editor de texto", "Metin düzenleyici", "محرر النصوص"),
        "nav_lib"      => ("Dokumentenablage", "Document tray", "Documentenlade", "Bandeja de documentos", "Belge tepsisi", "درج المستندات"),
        "settings"     => ("Einstellungen", "Settings", "Instellingen", "Configuración", "Ayarlar", "الإعدادات"),
        "close"        => ("Schließen", "Close", "Sluiten", "Cerrar", "Kapat", "إغلاق"),
        "ok"           => ("OK", "OK", "OK", "OK", "Tamam", "موافق"),
        "cancel"       => ("Abbrechen", "Cancel", "Annuleren", "Cancelar", "İptal", "إلغاء"),
        "save"         => ("Speichern", "Save", "Opslaan", "Guardar", "Kaydet", "حفظ"),
        "delete"       => ("Löschen", "Delete", "Verwijderen", "Eliminar", "Sil", "حذف"),
        "edit"         => ("Bearbeiten", "Edit", "Bewerken", "Editar", "Düzenle", "تعديل"),
        "reset"        => ("Zurücksetzen", "Reset", "Herstellen", "Restablecer", "Sıfırla", "إعادة تعيين"),
        "reset_all"    => ("Alles zurücksetzen", "Reset all", "Alles herstellen", "Restablecer todo", "Tümünü sıfırla", "إعادة تعيين الكل"),
        "on"           => ("An", "On", "Aan", "Activado", "Açık", "مفعّل"),
        "off"          => ("Aus", "Off", "Uit", "Desactivado", "Kapalı", "معطّل"),
        // --- Uebersetzer ----------------------------------------------------
        "translate"    => ("Übersetzen", "Translate", "Vertalen", "Traducir", "Çevir", "ترجم"),
        "swap"         => ("Tauschen", "Swap languages", "Wisselen", "Intercambiar", "Değiştir", "تبديل"),
        "reload"       => ("Modelle neu laden", "Reload models", "Modellen opnieuw laden", "Recargar modelos", "Modelleri yeniden yükle", "إعادة تحميل النماذج"),
        "input_ph"     => ("Text eingeben oder Datei hineinziehen …", "Type text or drop a file here …", "Typ tekst of sleep een bestand hierheen …", "Escribe el texto o arrastra un archivo aquí …", "Metin girin veya buraya bir dosya bırakın …", "اكتب نصاً أو أسقط ملفاً هنا …"),
        "attach"       => ("Datei anhängen", "Attach file", "Bestand toevoegen", "Adjuntar archivo", "Dosya ekle", "إرفاق ملف"),
        "copy"         => ("Kopieren", "Copy", "Kopiëren", "Copiar", "Kopyala", "نسخ"),
        "copied"       => ("Kopiert", "Copied", "Gekopieerd", "Copiado", "Kopyalandı", "تم النسخ"),
        "stop"         => ("Stoppen", "Stop", "Stoppen", "Detener", "Durdur", "إيقاف"),
        "stopped"      => ("Übersetzung angehalten", "Translation stopped", "Vertaling gestopt", "Traducción detenida", "Çeviri durduruldu", "تم إيقاف الترجمة"),
        "translating"  => ("Übersetze …", "Translating …", "Bezig met vertalen …", "Traduciendo …", "Çevriliyor …", "جاري الترجمة …"),
        "no_model"     => ("Kein Modell gewählt", "No model selected", "Geen model gekozen", "Ningún modelo elegido", "Model seçilmedi", "لم يتم اختيار نموذج"),
        "no_models"    => ("Keine Modelle gefunden", "No models found", "Geen modellen gevonden", "No se encontraron modelos", "Model bulunamadı", "لم يتم العثور على نماذج"),
        "wait_ollama"  => ("Warte auf Ollama …", "Waiting for Ollama …", "Wachten op Ollama …", "Esperando a Ollama …", "Ollama bekleniyor …", "في انتظار Ollama …"),
        "need_src"     => ("Dieses Modell braucht eine feste Ausgangssprache", "This model needs a fixed source language", "Dit model heeft een vaste brontaal nodig", "Este modelo necesita un idioma de origen fijo", "Bu model sabit bir kaynak dil gerektirir", "يحتاج هذا النموذج إلى لغة مصدر محددة"),
        "detect"       => ("Sprache erkennen", "Detect language", "Taal detecteren", "Detectar idioma", "Dili algıla", "تحديد تلقائي للغة"),
        "search_lang"  => ("Sprache suchen …", "Search language …", "Taal zoeken …", "Buscar idioma …", "Dil ara …", "بحث عن لغة …"),
        "no_hits"      => ("Keine Treffer", "No results", "Geen resultaten", "Sin resultados", "Sonuç bulunamadı", "لا توجد نتائج"),
        "pin"          => ("Anpinnen", "Pin", "Vastpinnen", "Fijar", "Sabitle", "تثبيت"),
        "style"        => ("Übersetzungsweise", "Translation style", "Vertaalstijl", "Estilo de traducción", "Çeviri stili", "أسلوب الترجمة"),
        "l_type"       => ("Art", "Type", "Type", "Tipo", "Tür", "النوع"),
        "l_formal"     => ("Ton", "Tone", "Toon", "Tono", "Ton", "النبرة"),
        "l_gender"     => ("Empfänger*in", "Addressee", "Ontvanger*", "Receptor*", "Alıcı*", "المستلم*"),
        "l_writer"     => ("Absender*in", "Writer", "Afzender*", "Remitente*", "Gönderen*", "المرسل*"),
        "o_natural"    => ("Natürlich", "Natural", "Natuurlijk", "Natural", "Doğal", "طبيعي"),
        "o_literal"    => ("Wörtlich", "Literal", "Letterlijk", "Literal", "Birebir", "حرفي"),
        "o_standard"   => ("Ohne Vorgabe", "Standard", "Standaard", "Estándar", "Standart", "قياسي"),
        "o_formal"     => ("Formell", "Formal", "Formeel", "Formal", "Resmi", "رسمي"),
        "o_informal"   => ("Ungezwungen", "Informal", "Informeel", "Informal", "Samimi", "غير رسمي"),
        "o_unknown"    => ("Unbekannt", "Unknown", "Onbekend", "Desconocido", "Bilinmiyor", "غير محدد"),
        "o_neutral"    => ("Nicht festgelegt", "Neutral", "Neutraal", "Neutro", "Nötr", "محايد"),
        "o_male"       => ("Männlich", "Male", "Mannelijk", "Masculino", "Erkek", "مذكر"),
        "o_female"     => ("Weiblich", "Female", "Vrouwelijk", "Femenino", "Kadın", "مؤنث"),
        "context"      => ("Zusammenhang", "Context", "Context", "Contexto", "Bağlam", "السياق"),
        "ctx_ph"       => ("z. B. „Text aus einer Bedienungsanleitung für eine Kaffeemaschine“", "e.g. “Text from a coffee machine manual”", "bijv. “Tekst uit de handleiding van een koffiemachine”", "p. ej. “Texto del manual de una cafetera”", "ör. “Bir kahve makinesi kılavuzundan metin”", "مثال: “نص من دليل مستخدم آلة القهوة”"),
        "glossary"     => ("Glossar", "Glossary", "Woordenlijst", "Glosario", "Sözlük", "المسرد"),
        "add_entry"    => ("+ Eintrag", "+ Entry", "+ Item", "+ Entrada", "+ Giriş", "+ إضافة"),
        "term"         => ("Begriff", "Term", "Term", "Término", "Kavram", "المصطلح"),
        "term_t"       => ("Übersetzung", "Translation", "Vertaling", "Traducción", "Çeviri", "الترجمة"),
        "remove"       => ("Entfernen", "Remove", "Verwijderen", "Eliminar", "Kaldır", "إزالة"),
        "note_nllb"    => ("NLLB-200 übersetzt direkt, ohne Wahl der Übersetzungsweise, Zusammenhang und Glossar. Diese Möglichkeiten gibt es nur mit allgemeinen Ollama-Modellen (z. B. Gemma 3).", "NLLB-200 translates directly, without style options, context and glossary. Those need a general-purpose Ollama model (e.g. Gemma 3).", "NLLB-200 vertaalt direct, zonder stijlopties, context en woordenlijst. Die zijn er alleen met algemene Ollama-modellen (bijv. Gemma 3).", "NLLB-200 traduce directamente, sin opciones de estilo, contexto ni glosario. Estas funciones solo están disponibles con modelos de Ollama de uso general (p. ej. Gemma 3).", "NLLB-200 doğrudan çevirir; stil, bağlam ve sözlük içermez. Bu özellikler sadece genel Ollama modellerinde (ör. Gemma 3) mevcuttur.", "يترجم NLLB-200 مباشرة دون خيارات الأسلوب أو السياق أو المسرد. هذه الميزات متاحة فقط مع نماذج Ollama العامة (مثل Gemma 3)."),
        "note_tg"      => ("TranslateGemma ist auf reine Übersetzung trainiert und folgt keinen Anweisungen zu Übersetzungsweise, Zusammenhang oder Glossar. Dafür bitte ein allgemeines Modell (z. B. gemma3:4b) wählen.", "TranslateGemma is trained purely for translation and ignores style, context and glossary instructions. Choose a general-purpose model (e.g. gemma3:4b) for those.", "TranslateGemma is puur getraind op vertalen en negeert stijl-, context- en woordenlijstinstructies. Kies daarvoor een algemeen model (bijv. gemma3:4b).", "TranslateGemma está entrenado puramente para traducir e ignora las instrucciones de estilo, contexto y glosario. Elige un modelo general (p. ej. gemma3:4b).", "TranslateGemma saf çeviri için eğitilmiştir ve stil/bağlam/sözlük yönergelerini dikkate almaz. Bunlar için genel bir model (ör. gemma3:4b) seçin.", "تم تدريب TranslateGemma للترجمة البحتة ويتجاهل تعليمات الأسلوب والسياق. اختر نموذجاً عاماً (مثل gemma3:4b)."),
        "chars"        => ("Zeichen", "characters", "tekens", "caracteres", "karakter", "حرف"),
        // --- Einstellungen --------------------------------------------------
        "s_group_general" => ("Allgemein", "General", "Algemeen", "General", "Genel", "عام"),
        "s_group_conn"    => ("Verbindungen", "Connections", "Verbindingen", "Conexiones", "Bağlantılar", "الاتصالات"),
        "s_group_trans"   => ("Übersetzung", "Translation", "Vertaling", "Traducción", "Çeviri", "الترجمة"),
        "s_group_extra"   => ("Weitere Einstellungen", "Additional settings", "Extra instellingen", "Ajustes adicionales", "Ek ayarlar", "إعدادات إضافية"),
        "s_uilang"     => ("Anzeigesprache", "Display language", "Weergavetaal", "Idioma de visualización", "Arayüz dili", "لغة الواجهة"),
        "s_theme"      => ("Darstellung", "Theme", "Thema", "Tema", "Tema", "المظهر"),
        "s_start"      => ("Beim Hochfahren öffnen", "Open at startup", "Openen bij starten", "Abrir al inicio", "Başlangıçta aç", "الفتح عند البدء"),
        "s_workspace"  => ("Sichtbare Arbeitsplätze", "Visible workspaces", "Zichtbare werkplekken", "Espacios visibles", "Görünür çalışma alanları", "مساحات العمل المرئية"),
        "s_url"        => ("Ollama-Adresse", "Ollama address", "Ollama-adres", "Dirección de Ollama", "Ollama adresi", "عنوان Ollama"),
        "s_nllb"       => ("NLLB-Server-Adresse", "NLLB server address", "Adres van NLLB-server", "Dirección del servidor NLLB", "NLLB sunucu adresi", "عنوان خادم NLLB"),
        "s_helper"     => ("Modell für Ersatzwörter (bei TranslateGemma/NLLB)", "Model for word alternatives (for TranslateGemma/NLLB)", "Model voor woordalternatieven", "Modelo para alternativas", "Kelime alternatifleri modeli", "نموذج بدائل الكلمات"),
        "s_sys"        => ("System-Anweisung", "System prompt", "Systeemprompt", "Prompt del sistema", "Sistem komutu", "توجيه النظام"),
        "s_temp"       => ("Temperatur (niedriger = beständiger)", "Temperature (lower = more consistent)", "Temperatuur (lager = consistenter)", "Temperatura (menor = más consistente)", "Sıcaklık (düşük = daha tutarlı)", "درجة الحرارة (أقل = أكثر دقة)"),
        "s_ctx"        => ("Kontextlänge (num_ctx)", "Context length (num_ctx)", "Contextlengte (num_ctx)", "Longitud de contexto (num_ctx)", "Bağlam uzunluğu (num_ctx)", "طول السياق (num_ctx)"),
        "s_boot"       => ("Letztes Modell beim Hochfahren vorwärmen", "Preload last used model at startup", "Laatst gebruikte model voorverwarmen", "Precargar último modelo al iniciar", "Son modeli ön yükle", "تحميل النموذج مسبقاً"),
        "s_prof"       => ("Schimpfwortfilter aktivieren", "Enable profanity filter", "Vloekfilter inschakelen", "Filtro de malas palabras", "Küfür filtresini etkinleştir", "فلتر الكلمات البذيئة"),
        "s_profstyle"  => ("Ersatz für Schimpfwörter", "Profanity replacement", "Vervanging voor vloekwoorden", "Sustitución de palabras malsonantes", "Küfürlerin değiştirilmesi", "استبدال الكلمات البذيئة"),
        "s_md"         => ("Dynamisches Markdown", "Dynamic Markdown", "Dynamische Markdown", "Markdown dinámico", "Dinamik Markdown", "ماركداون ديناميكي"),
        "s_spellanim"  => ("Rechtschreibwellen schlängeln", "Animate spelling squiggles", "Spellingsgolven animeren", "Animar las ondulaciones", "Yazım dalgalarını canlandır", "تحريك موجات التدقيق"),
        "s_autocopy"   => ("Ausgabe automatisch kopieren", "Copy output automatically", "Uitvoer automatisch kopiëren", "Copiar la salida automáticamente", "Çıktıyı otomatik kopyala", "نسخ الناتج تلقائياً"),
        "s_autocopy_t" => ("Ausgabe nur bei getippter Eingabe kopieren", "Copy output only for typed input", "Alleen bij getypte invoer kopiëren", "Copiar solo con texto tecleado", "Yalnızca yazılan metinde kopyala", "نسخ الناتج فقط عند الكتابة"),
        "start_tr"     => ("Übersetzer öffnen", "Open translator", "Vertaler openen", "Abrir traductor", "Çevirmeni aç", "فتح المترجم"),
        "start_ed"     => ("Textbearbeitung öffnen", "Open text editor", "Tekstbewerking openen", "Abrir editor", "Editörü aç", "فتح المحرر"),
        "start_last"   => ("Zuletzt verwendetes öffnen", "Open last used", "Laatst gebruikte openen", "Abrir último usado", "Son kullanılanı aç", "فتح آخر مستخدم"),
        "ws_both"      => ("Beide", "Both", "Beide", "Ambos", "Her ikisi", "كلاهما"),
        "ws_tr"        => ("Nur Übersetzer", "Translator only", "Alleen vertaler", "Solo traductor", "Yalnızca çevirmen", "المترجم فقط"),
        "ws_ed"        => ("Nur Textbearbeitung", "Text editor only", "Alleen tekstbewerking", "Solo editor", "Yalnızca editör", "المحرر فقط"),
        "auto_sys"     => ("Automatisch (System)", "Automatic (system)", "Automatisch (systeem)", "Automático (sistema)", "Otomatik (sistem)", "تلقائي (النظام)"),
        "auto_o"       => ("Automatisch", "Automatic", "Automatisch", "Automático", "Otomatik", "تلقائي"),
        "auto_model"   => ("Automatisch (Programm wählt)", "Automatic (app chooses)", "Automatisch (programma kiest)", "Automático (la aplicación elige)", "Otomatik (uygulama seçer)", "تلقائي (التطبيق يختار)"),
        "light"        => ("Hell", "Light", "Licht", "Claro", "Açık", "فاتح"),
        "dark"         => ("Dunkel", "Dark", "Donker", "Oscuro", "Koyu", "داكن"),
        "o_stars"      => ("Sternchen (****)", "Asterisks (****)", "Sterretjes (****)", "Asteriscos (****)", "Yıldızlar (****)", "نجوم (****)"),
        "o_symbols"    => ("Sonderzeichen (@#$%&!)", "Symbols (@#$%&!)", "Symbolen (@#$%&!)", "Símbolos (@#$%&!)", "Semboller (@#$%&!)", "رموز (@#$%&!)"),
        // --- Editor ---------------------------------------------------------
        "new_doc"      => ("Neu", "New", "Nieuw", "Nuevo", "Yeni", "جديد"),
        "open"         => ("Öffnen", "Open", "Openen", "Abrir", "Aç", "فتح"),
        "save_to_tray" => ("In der Ablage speichern", "Save to tray", "In de lade opslaan", "Guardar en la bandeja", "Tepsiye kaydet", "حفظ في الدرج"),
        "export"       => ("Exportieren", "Export", "Exporteren", "Exportar", "Dışa aktar", "تصدير"),
        "saved"        => ("Gespeichert", "Saved", "Opgeslagen", "Guardado", "Kaydedildi", "تم الحفظ"),
        "words"        => ("Wörter", "words", "woorden", "palabras", "kelime", "كلمات"),
        "doc_name"     => ("Dokument", "Document", "Document", "Documento", "Belge", "مستند"),
        "lib_empty"    => ("Noch keine Dokumente in der Ablage.", "No documents in the tray yet.", "Nog geen documenten in de lade.", "Aún no hay documentos en la bandeja.", "Tepside henüz belge yok.", "لا توجد مستندات في الدرج بعد."),
        "lib_empty_hint" => ("Speichern Sie ein Dokument im Texteditor, um es in der Ablage wiederzufinden.", "Save a document in the text editor to find it in the tray.", "Sla een document op om het terug te vinden.", "Guarda un documento para encontrarlo en la bandeja.", "Burada bulmak için bir belge kaydedin.", "احفظ مستنداً لتجده في الدرج."),
        "lib_delete_q" => ("Dieses Dokument aus der Ablage entfernen?", "Remove this document from the tray?", "Dit document uit de lade verwijderen?", "¿Quitar este documento de la bandeja?", "Bu belge tepside kaldırılsın mı?", "إزالة هذا المستند من الدرج؟"),
        "redo"         => ("Wiederholen", "Redo", "Opnieuw", "Rehacer", "Yinele", "إعادة"),
        "bold"         => ("Fett", "Bold", "Vet", "Negrita", "Kalın", "عريض"),
        "italic"       => ("Kursiv", "Italic", "Cursief", "Cursiva", "İtalik", "مائل"),
        "underline"    => ("Unterstrichen", "Underline", "Onderstreept", "Subrayado", "Altı çizili", "تحته خط"),
        "strike"       => ("Durchgestrichen", "Strikethrough", "Doorstreept", "Tachado", "Üstü çizili", "مشطوب"),
        "clear_fmt"    => ("Formatierung entfernen", "Clear formatting", "Opmaak wissen", "Quitar formato", "Biçimi temizle", "إزالة التنسيق"),
        "list"          => ("Liste", "List", "Lijst", "Lista", "Liste", "قائمة"),
        "quote"         => ("Zitat", "Quote", "Citaat", "Cita", "Alıntı", "اقتباس"),
        "link"          => ("Link", "Link", "Link", "Enlace", "Bağlantı", "رابط"),
        "session_docs"  => ("Geöffnete Dokumente", "Open documents", "Geopende documenten", "Documentos abiertos", "Açık belgeler", "المستندات المفتوحة"),
        "close_session" => ("Aus der Sitzung schließen", "Close from session", "Uit sessie sluiten", "Cerrar de la sesión", "Oturumdan kapat", "إغلاق من الجلسة"),
        "discard_q"     => ("Ungespeicherte Änderungen verwerfen?", "Discard unsaved changes?", "Niet-opgeslagen wijzigingen verwerpen?", "¿Descartar los cambios sin guardar?", "Kaydedilmemiş değişiklikler atılsın mı?", "تجاهل التغييرات غير المحفوظة؟"),
        "diff_title"    => ("Probelesen", "Proofread", "Proeflezen", "Revisión", "Düzeltme", "تدقيق"),
        "accept_all"    => ("Alle übernehmen", "Accept all", "Alles overnemen", "Aceptar todo", "Tümünü kabul et", "قبول الكل"),
        "discard_all"   => ("Alle verwerfen", "Discard all", "Alles verwerpen", "Descartar todo", "Tümünü at", "تجاهل الكل"),
        "accept"        => ("Übernehmen", "Accept", "Overnemen", "Aceptar", "Kabul et", "قبول"),
        "discard"       => ("Verwerfen", "Discard", "Verwerpen", "Descartar", "At", "تجاهل"),
        "apply"         => ("Anwenden", "Apply", "Toepassen", "Aplicar", "Uygula", "تطبيق"),
        "diff_empty"    => ("Keine Unterschiede", "No differences", "Geen verschillen", "Sin diferencias", "Fark yok", "لا فروق"),
        "diff_hunks"    => ("Änderungen", "Changes", "Wijzigingen", "Cambios", "Değişiklikler", "التغييرات"),
        "shorten"      => ("Verkürzen", "Shorten", "Verkorten", "Acortar", "Kısalt", "اختصار"),
        "lengthen"     => ("Verlängern", "Lengthen", "Verlengen", "Alargar", "Uzat", "إطالة"),
        "summarize"    => ("Zusammenfassen", "Summarize", "Samenvatten", "Resumir", "Özetle", "تلخيص"),
        "proofread"    => ("Probelesen", "Proofread", "Proeflezen", "Revisar", "Düzelt", "تدقيق"),
        "synonyms"     => ("Synonyme", "Synonyms", "Synoniemen", "Sinónimos", "Eş anlamlılar", "مرادفات"),
        "undo"         => ("Rückgängig", "Undo", "Ongedaan maken", "Deshacer", "Geri al", "تراجع"),
        "font"         => ("Schrift", "Font", "Lettertype", "Fuente", "Yazı tipi", "الخط"),
        "size"         => ("Größe", "Size", "Grootte", "Tamaño", "Boyut", "الحجم"),
        "heading"      => ("Absatz", "Paragraph", "Alinea", "Párrafo", "Paragraf", "فقرة"),
        "personal_dict" => ("Eigenes Wörterbuch", "Personal dictionary", "Persoonlijk woordenboek", "Diccionario personal", "Kişisel sözlük", "القاموس الشخصي"),
        "dict_words"   => ("Gespeicherte Wörter", "Saved words", "Opgeslagen woorden", "Palabras guardadas", "Kayıtlı kelimeler", "الكلمات المحفوظة"),
        "add_word"     => ("Wort hinzufügen", "Add word", "Woord toevoegen", "Añadir palabra", "Kelime ekle", "إضافة كلمة"),
        "word_ph"      => ("Neues Wort eingeben …", "Type new word …", "Nieuw woord typen …", "Nueva palabra …", "Yeni kelime yazın …", "اكتب كلمة جديدة …"),
        "no_dict_words" => ("Keine Wörter im persönlichen Wörterbuch", "No words in personal dictionary", "Geen woorden in persoonlijk woordenboek", "No hay palabras en el diccionario", "Kişisel sözlükte kayıtlı kelime yok", "لا توجد كلمات في القاموس"),
        "word_exists"  => ("Wort existiert bereits", "Word already exists", "Woord bestaat al", "La palabra ya existe", "Kelime zaten mevcut", "الكلمة موجودة بالفعل"),
        "spell_on"     => ("Rechtschreibung prüfen", "Check spelling", "Spelling controleren", "Revisar ortografía", "Yazımı denetle", "تدقيق الإملاء"),
        "spell_title"   => ("Rechtschreibung", "Spelling", "Spelling", "Ortografía", "Yazım", "الإملاء"),
        "spell_missing" => ("Wörterbuch nicht gefunden", "Dictionary not found", "Woordenboek niet gevonden", "Diccionario no encontrado", "Sözlük bulunamadı", "القاموس غير موجود"),
        "spell_missing_hint" => ("Legen Sie index.aff und index.dic unter dict/<sprache>/ ab (oder installieren Sie das npm-Paket dictionary-<sprache>).", "Place index.aff and index.dic under dict/<language>/ (or install the npm package dictionary-<language>).", "Plaats index.aff en index.dic onder dict/<taal>/ (of installeer dictionary-<taal>).", "Coloque index.aff e index.dic en dict/<idioma>/.", "index.aff ve index.dic dosyalarını dict/<dil>/ altına koyun.", "ضع index.aff و index.dic تحت dict/<اللغة>/."),
        "spell_recheck" => ("Erneut prüfen", "Check again", "Opnieuw controleren", "Comprobar de nuevo", "Yeniden denetle", "إعادة التدقيق"),
        "spell_no_errors" => ("Keine Fehler gefunden", "No errors found", "Geen fouten gevonden", "No se encontraron errores", "Hata bulunamadı", "لم يتم العثور على أخطاء"),
        "ignore"       => ("Ignorieren", "Ignore", "Negeren", "Ignorar", "Yoksay", "تجاهل"),
        "add_to_dict"  => ("Ins Wörterbuch", "Add to dictionary", "Aan woordenboek toevoegen", "Añadir al diccionario", "Sözlüğe ekle", "إضافة إلى القاموس"),
        "suggestions"  => ("Vorschläge", "Suggestions", "Suggesties", "Sugerencias", "Öneriler", "اقتراحات"),
        "replace_all"  => ("Alle ersetzen", "Replace all", "Alles vervangen", "Reemplazar todo", "Tümünü değiştir", "استبدال الكل"),
        "dict"         => ("Wörterbuchsprache", "Dictionary language", "Woordenboektaal", "Idioma del diccionario", "Sözlük dili", "لغة القاموس"),
        "error"        => ("Fehler", "Error", "Fout", "Error", "Hata", "خطأ"),
        "server_down"  => ("Der Übersetzungs-Server ist nicht erreichbar. Er wird bei der Einrichtung (Installieren.bat) miteingerichtet und läuft mit dem Programm an.", "The translation server is not reachable. It is installed by the setup wizard (Installieren.bat) and starts with the app.", "De vertaalserver is niet bereikbaar. Die wordt door de installatiewizard geïnstalleerd.", "El servidor de traducción no está disponible. Se instala con el asistente.", "Çeviri sunucusuna ulaşılamıyor. Kurulum sihirbazı ile kurulur.", "خادم الترجمة غير متاح. يتم تثبيته عبر المعالج."),
        "nllb_h"       => ("NLLB-200", "NLLB-200", "NLLB-200", "NLLB-200", "NLLB-200", "NLLB-200"),
        "installed"    => ("Eingerichtet", "Installed", "Geïnstalleerd", "Instalado", "Yüklü", "مثبت"),
        "not_installed" => ("Nicht eingerichtet", "Not installed", "Niet geïnstalleerd", "No instalado", "Yüklü değil", "غير مثبت"),
        "download"     => ("Herunterladen", "Download", "Downloaden", "Descargar", "İndir", "تنزيل"),
        "server_langs" => ("Sprachen des Servers", "Server languages", "Talen van de server", "Idiomas del servidor", "Sunucu dilleri", "لغات الخادم"),
        _ => (LEAK, LEAK, LEAK, LEAK, LEAK, LEAK),
    }
}

/// Platzhalter, damit unbekannte Schluessel sichtbar bleiben.
const LEAK: &str = "";

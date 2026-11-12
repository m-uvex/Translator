mod app;
mod editor;
mod engine;
mod fenster;
mod pdf;
mod icons;
mod icons_data;
mod i18n;
mod library;
mod net;
mod nllb;
mod settings;
mod spell;

use eframe::egui;

/// Laedt Windows-Schriften nach, damit Umlaute, Kana und arabische Zeichen
/// sicher dargestellt werden. Fehlt die Datei, bleibt egui bei seinen
/// eingebauten Schriften.
///
/// `seguisym` ist wichtig fuer die Oberflaeche selbst: Die Knopfsymbole
/// (✕, ✎, ▤, ⚙) sind Zeichen, die weder Segoe UI noch Arial noch die
/// eingebaute egui-Schrift enthalten. Ohne diese Schrift erscheinen sie als
/// leere Kaestchen.
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut namen: Vec<String> = Vec::new();
    // Reihenfolge = Vorrang. Segoe UI ist die Schrift der Electron-Fassung;
    // die eingebauten egui-Schriften bleiben als Rueckfall erhalten.
    // Die Schriftnamen stehen fuer den Nutzer da, nicht fuer den Rechner: Die
    // Sprachenliste zeigt Koreanisch, Japanisch, Chinesisch und Arabisch in
    // eigener Schrift, sonst erscheinen dort leere Kaestchen. Die .ttc-Dateien
    // sind Sammlungen; `index` waehlt die erste Schrift darin.
    for (name, path, index) in [
        ("segoe", "C:/Windows/Fonts/segoeui.ttf", 0),
        ("segoeui", "C:/Windows/Fonts/segoeui.ttf", 0),
        ("arial", "C:/Windows/Fonts/arial.ttf", 0),
        // Nur als letzter Rueckfall fuer einzelne Zeichen wie ▾ oder → gedacht;
        // die Knopfsymbole sind Vektorgrafik und brauchen sie nicht.
        ("seguisym", "C:/Windows/Fonts/seguisym.ttf", 0),
        ("malgun", "C:/Windows/Fonts/malgun.ttf", 0),
        ("yugoth", "C:/Windows/Fonts/YuGothR.ttc", 0),
        ("yahei", "C:/Windows/Fonts/msyh.ttc", 0),
    ] {
        if namen.iter().any(|n| n == name) {
            continue;
        }
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(
                name.to_string(),
                egui::FontData {
                    font: bytes.into(),
                    index,
                    tweak: Default::default(),
                },
            );
            namen.push(name.to_string());
        }
    }
    if !namen.is_empty() {
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            let list = fonts.families.entry(family).or_default();
            for name in namen.iter().rev() {
                list.insert(0, name.clone());
            }
        }
        ctx.set_fonts(fonts);
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([960.0, 600.0])
            .with_decorations(false)
            .with_title("Übersetzer"),
        ..Default::default()
    };
    eframe::run_native(
        "Übersetzer",
        options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            // Abgerundete Fensterecken wie bei jedem anderen Windows-Programm.
            fenster::runde_ecken();
            Box::new(app::App::new(cc))
        }),
    )
}

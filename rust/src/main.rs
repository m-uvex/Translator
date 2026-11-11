mod app;
mod engine;
mod i18n;
mod library;
mod net;
mod settings;

use eframe::egui;

/// Laedt Windows-Schriften nach, damit Umlaute, Kana und arabische Zeichen
/// sicher dargestellt werden. Fehlt die Datei, bleibt egui bei seinen
/// eingebauten Schriften.
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut loaded = false;
    for (name, path) in [
        ("segoe", "C:/Windows/Fonts/segoeui.ttf"),
        ("arial", "C:/Windows/Fonts/arial.ttf"),
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(name.to_string(), egui::FontData::from_owned(bytes));
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(family).or_default().push(name.to_string());
            }
            loaded = true;
        }
    }
    if loaded {
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
            Box::new(app::App::new(cc))
        }),
    )
}

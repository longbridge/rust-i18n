#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui::{self, TextBuffer};
use rust_i18n::t;

rust_i18n::i18n!(
    "locales",
    minify_key = true,
    minify_key_len = 12,
    minify_key_prefix = "tr_",
    minify_key_thresh = 8
);

fn main() -> Result<(), eframe::Error> {
    #[cfg(feature = "log-miss-tr")]
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    eframe::run_native(
        t!("My egui App").as_str(),
        options,
        Box::new(|cc| {
            // This gives us image support:
            // egui_extras::install_image_loaders(&cc.egui_ctx);
            let _ = setup_custom_fonts(&cc.egui_ctx);
            Box::<MyApp>::default()
        }),
    )
}

struct MyApp {
    name: String,
    age: u32,
    locale_id: usize,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            name: t!("Arthur").into(),
            age: 42,
            locale_id: 0,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(t!("My egui Application"));
            ui.horizontal(|ui| {
                let name_label = ui.label(t!("Your name: "));
                ui.text_edit_singleline(&mut self.name)
                    .labelled_by(name_label.id);
            });
            ui.add(egui::Slider::new(&mut self.age, 0..=120).text(t!("age")));
            if ui.button(t!("Click each year")).clicked() {
                self.age += 1;
            }
            ui.label(t!("Hello '%{name}', age %{age}", name => self.name, age => self.age));

            ui.separator();

            ui.horizontal(|ui| {
                let locales = rust_i18n::available_locales!();
                for (i, locale) in locales.iter().enumerate() {
                    if ui
                        .selectable_value(&mut self.locale_id, i, locale.clone())
                        .changed()
                    {
                        rust_i18n::set_locale(locale);
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(
                            t!("My egui App").to_string(),
                        ));
                    }
                }
            });
        });
    }
}

#[cfg(windows)]
fn try_load_system_font() -> Result<Vec<u8>, std::io::Error> {
    let font_files = &[
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/msjh.ttf",
        "C:/Windows/Fonts/yugothr.ttc",
        "C:/Windows/Fonts/malgun.ttf",
    ];

    for font in font_files {
        if let Ok(font) = std::fs::read(font) {
            return Ok(font);
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "No system font found",
    ))
}

#[cfg(windows)]
fn setup_custom_fonts(ctx: &egui::Context) -> Result<(), std::io::Error> {
    // Start with the default fonts (we will be adding to them rather than replacing them).
    let mut fonts = egui::FontDefinitions::default();

    // Install my own font (maybe supporting non-latin characters).
    // .ttf and .otf files supported.
    let font_bytes = try_load_system_font()?;
    fonts
        .font_data
        .insert("my_font".to_owned(), egui::FontData::from_owned(font_bytes));

    // Put my font first (highest priority) for proportional text:
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "my_font".to_owned());

    // Put my font as last fallback for monospace:
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("my_font".to_owned());

    // Tell egui to use these fonts:
    ctx.set_fonts(fonts);

    Ok(())
}

#[cfg(not(windows))]
fn setup_custom_fonts(_ctx: &egui::Context) -> Result<(), std::io::Error> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Custom fonts not supported on this platform",
    ))
}

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;

use crate::config::AppConfig;
use crate::tasks::{
    LogFn, Provider, Quality, ToDocxParams, ToHtmlParams, ToJsonParams, ToPdfParams,
};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(PartialEq)]
enum Tab {
    Convert,
    Generate,
    Validate,
    Settings,
}

#[derive(PartialEq, Clone, Copy)]
enum GenFormat {
    Docx,
    Html,
    Pdf,
}

pub struct SdsApp {
    config: AppConfig,
    rt: tokio::runtime::Runtime,
    log: Arc<Mutex<Vec<String>>>,
    busy: Arc<AtomicBool>,
    tab: Tab,

    // Convert tab
    conv_input: String,
    conv_output: String,
    conv_provider: String,
    conv_quality: String,
    conv_lang: String,
    conv_enrich: bool,

    // Generate tab
    gen_input: String,
    gen_output: String,
    gen_format: GenFormat,
    gen_lang: String,

    // Validate tab
    val_input: String,
    val_results: Vec<String>,
    val_pending: Arc<Mutex<Option<Vec<String>>>>,

    // Settings tab
    settings_saved_msg: Option<String>,
}

impl SdsApp {
    pub fn new() -> Self {
        let config = AppConfig::load();
        Self {
            conv_provider: config.provider.clone(),
            conv_quality: config.quality.clone(),
            conv_lang: config.language.clone(),
            gen_lang: config.language.clone(),
            config,
            rt: tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
            log: Arc::new(Mutex::new(Vec::new())),
            busy: Arc::new(AtomicBool::new(false)),
            tab: Tab::Convert,
            conv_input: String::new(),
            conv_output: String::new(),
            conv_enrich: false,
            gen_input: String::new(),
            gen_output: String::new(),
            gen_format: GenFormat::Docx,
            val_input: String::new(),
            val_results: Vec::new(),
            val_pending: Arc::new(Mutex::new(None)),
            settings_saved_msg: None,
        }
    }

    fn log(&self, msg: impl Into<String>) {
        if let Ok(mut v) = self.log.lock() {
            v.push(msg.into());
        }
    }

    fn make_log_fn(&self) -> LogFn {
        let log = Arc::clone(&self.log);
        Arc::new(move |msg| {
            if let Ok(mut v) = log.lock() {
                v.push(msg);
            }
        })
    }

    fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Relaxed)
    }

    // --- tab UIs ---

    fn ui_convert_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("SDS文書 → MHLW標準JSON");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("入力 (ファイル/URL):");
            ui.add_sized([320.0, 20.0], egui::TextEdit::singleline(&mut self.conv_input));
            if ui.button("参照...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SDS文書", &["pdf", "docx", "xlsx", "txt", "html"])
                    .pick_file()
                {
                    self.conv_input = path.to_string_lossy().into_owned();
                    // Auto-suggest output path
                    if self.conv_output.is_empty() {
                        if let Some(stem) = path.file_stem() {
                            let mut out = path.clone();
                            out.set_file_name(format!("{}.json", stem.to_string_lossy()));
                            self.conv_output = out.to_string_lossy().into_owned();
                        }
                    }
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("出力 JSON:            ");
            ui.add_sized([320.0, 20.0], egui::TextEdit::singleline(&mut self.conv_output));
            if ui.button("保存先...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("JSON", &["json"])
                    .save_file()
                {
                    self.conv_output = path.to_string_lossy().into_owned();
                }
            }
        });

        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.label("プロバイダ:");
            egui::ComboBox::from_id_salt("conv_provider")
                .selected_text(&self.conv_provider)
                .width(140.0)
                .show_ui(ui, |ui| {
                    for &p in Provider::all() {
                        ui.selectable_value(&mut self.conv_provider, p.to_string(), p);
                    }
                });

            ui.add_space(12.0);
            ui.label("品質:");
            egui::ComboBox::from_id_salt("conv_quality")
                .selected_text(&self.conv_quality)
                .width(90.0)
                .show_ui(ui, |ui| {
                    for &q in Quality::all() {
                        ui.selectable_value(&mut self.conv_quality, q.to_string(), q);
                    }
                });

            ui.add_space(12.0);
            ui.label("言語:");
            lang_combo(ui, "conv_lang", &mut self.conv_lang);
        });

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.conv_enrich, "PubChem照合 (--enrich)");
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            let btn = egui::Button::new(if self.is_busy() { "変換中..." } else { "変換開始" });
            if ui.add_enabled(!self.is_busy(), btn).clicked() {
                self.start_convert(ctx);
            }
            if self.is_busy() {
                ui.spinner();
            }
        });
    }

    fn start_convert(&mut self, ctx: &egui::Context) {
        let input   = self.conv_input.trim().to_string();
        let output  = PathBuf::from(self.conv_output.trim());
        let provider = Provider::from_str(&self.conv_provider);
        let quality  = Quality::from_str(&self.conv_quality);
        let lang     = lang_from_str(&self.conv_lang);
        let enrich   = self.conv_enrich;

        // Resolve API key: config file → env var
        let api_key = self.config.api_key.clone();
        let api_key = if api_key.is_empty() {
            std::env::var(provider.api_key_env()).unwrap_or_default()
        } else {
            api_key
        };
        if api_key.is_empty() {
            self.log(format!(
                "[ERROR] APIキーが設定されていません。設定タブで入力してください ({})",
                provider.api_key_env()
            ));
            return;
        }
        if input.is_empty() {
            self.log("[ERROR] 入力ファイルを指定してください".to_string());
            return;
        }
        let model = provider.default_model(quality).to_string();
        let log_fn    = self.make_log_fn();
        let log_err   = Arc::clone(&self.log);
        let busy      = Arc::clone(&self.busy);
        let ctx2      = ctx.clone();
        busy.store(true, Ordering::Relaxed);
        self.log(format!("[START] {} → {}", input, output.display()));
        self.rt.spawn(async move {
            if let Err(e) = crate::tasks::run_to_json(ToJsonParams {
                input, output, provider, api_key, model, quality, lang, base_url: None, enrich,
            }, log_fn).await {
                if let Ok(mut v) = log_err.lock() {
                    v.push(format!("[ERROR] {e}"));
                }
            }
            busy.store(false, Ordering::Relaxed);
            ctx2.request_repaint();
        });
    }

    fn ui_generate_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("MHLW JSON → 文書生成");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("入力 JSON:");
            ui.add_sized([320.0, 20.0], egui::TextEdit::singleline(&mut self.gen_input));
            if ui.button("参照...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("JSON", &["json"])
                    .pick_file()
                {
                    self.gen_input = path.to_string_lossy().into_owned();
                    if self.gen_output.is_empty() {
                        let ext = match self.gen_format {
                            GenFormat::Docx => "docx",
                            GenFormat::Html => "html",
                            GenFormat::Pdf  => "pdf",
                        };
                        if let Some(stem) = path.file_stem() {
                            let mut out = path.clone();
                            out.set_file_name(format!("{}.{ext}", stem.to_string_lossy()));
                            self.gen_output = out.to_string_lossy().into_owned();
                        }
                    }
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("出力ファイル:");
            ui.add_sized([320.0, 20.0], egui::TextEdit::singleline(&mut self.gen_output));
            if ui.button("保存先...").clicked() {
                let (desc, ext) = match self.gen_format {
                    GenFormat::Docx => ("Word文書", vec!["docx"]),
                    GenFormat::Html => ("HTML", vec!["html"]),
                    GenFormat::Pdf  => ("PDF", vec!["pdf"]),
                };
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(desc, &ext)
                    .save_file()
                {
                    self.gen_output = path.to_string_lossy().into_owned();
                }
            }
        });

        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.label("形式:");
            ui.selectable_value(&mut self.gen_format, GenFormat::Docx, "DOCX");
            ui.selectable_value(&mut self.gen_format, GenFormat::Html, "HTML");
            ui.selectable_value(&mut self.gen_format, GenFormat::Pdf, "PDF");

            ui.add_space(12.0);
            ui.label("言語:");
            lang_combo(ui, "gen_lang", &mut self.gen_lang);
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            let btn = egui::Button::new(if self.is_busy() { "生成中..." } else { "生成開始" });
            if ui.add_enabled(!self.is_busy(), btn).clicked() {
                self.start_generate(ctx);
            }
            if self.is_busy() {
                ui.spinner();
            }
        });
    }

    fn start_generate(&mut self, ctx: &egui::Context) {
        let input  = PathBuf::from(self.gen_input.trim());
        let output = PathBuf::from(self.gen_output.trim());
        let lang   = lang_from_str(&self.gen_lang)
            .unwrap_or(sds_converter_core::Language::Japanese);
        let format = self.gen_format;

        if self.gen_input.is_empty() {
            self.log("[ERROR] 入力JSONファイルを指定してください".to_string());
            return;
        }

        let log_fn  = self.make_log_fn();
        let log_err = Arc::clone(&self.log);
        let busy    = Arc::clone(&self.busy);
        let ctx2    = ctx.clone();
        busy.store(true, Ordering::Relaxed);
        self.log(format!("[START] {} → {}", input.display(), output.display()));

        self.rt.spawn(async move {
            let result = match format {
                GenFormat::Docx => {
                    crate::tasks::run_to_docx(
                        ToDocxParams { input, output, lang, template: None },
                        log_fn,
                    ).await
                }
                GenFormat::Html => {
                    crate::tasks::run_to_html(
                        ToHtmlParams { input, output, lang },
                        log_fn,
                    ).await
                }
                GenFormat::Pdf => {
                    crate::tasks::run_to_pdf(
                        ToPdfParams { input, output, lang },
                        log_fn,
                    ).await
                }
            };
            if let Err(e) = result {
                if let Ok(mut v) = log_err.lock() {
                    v.push(format!("[ERROR] {e}"));
                }
            }
            busy.store(false, Ordering::Relaxed);
            ctx2.request_repaint();
        });
    }

    fn ui_validate_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("JSONバリデーション");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("入力 JSON:");
            ui.add_sized([320.0, 20.0], egui::TextEdit::singleline(&mut self.val_input));
            if ui.button("参照...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("JSON", &["json"])
                    .pick_file()
                {
                    self.val_input = path.to_string_lossy().into_owned();
                }
            }
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            let btn = egui::Button::new(if self.is_busy() { "検証中..." } else { "検証実行" });
            if ui.add_enabled(!self.is_busy(), btn).clicked() {
                self.start_validate(ctx);
            }
            if self.is_busy() {
                ui.spinner();
            }
        });

        if !self.val_results.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for w in &self.val_results {
                    let color = if w.starts_with("OK") {
                        egui::Color32::GREEN
                    } else {
                        egui::Color32::YELLOW
                    };
                    ui.colored_label(color, w);
                }
            });
        }
    }

    fn start_validate(&mut self, ctx: &egui::Context) {
        let input = PathBuf::from(self.val_input.trim());
        if self.val_input.is_empty() {
            self.log("[ERROR] 入力JSONファイルを指定してください".to_string());
            return;
        }
        self.val_results.clear();
        let log_fn  = self.make_log_fn();
        let busy    = Arc::clone(&self.busy);
        let ctx2    = ctx.clone();
        let pending = Arc::clone(&self.val_pending);
        busy.store(true, Ordering::Relaxed);

        self.rt.spawn(async move {
            let results = match crate::tasks::run_validate(input, log_fn).await {
                Ok(warnings) if warnings.is_empty() => {
                    vec!["OK: 問題は見つかりませんでした".to_string()]
                }
                Ok(warnings) => warnings,
                Err(e) => vec![format!("[ERROR] {e}")],
            };
            if let Ok(mut slot) = pending.lock() {
                *slot = Some(results);
            }
            busy.store(false, Ordering::Relaxed);
            ctx2.request_repaint();
        });
    }

    fn ui_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("設定");
        ui.add_space(8.0);

        egui::Grid::new("settings_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("デフォルトプロバイダ:");
                egui::ComboBox::from_id_salt("settings_provider")
                    .selected_text(&self.config.provider)
                    .width(140.0)
                    .show_ui(ui, |ui| {
                        for &p in Provider::all() {
                            ui.selectable_value(&mut self.config.provider, p.to_string(), p);
                        }
                    });
                ui.end_row();

                ui.label("デフォルト言語:");
                lang_combo(ui, "settings_lang", &mut self.config.language);
                ui.end_row();

                ui.label("デフォルト品質:");
                egui::ComboBox::from_id_salt("settings_quality")
                    .selected_text(&self.config.quality)
                    .width(90.0)
                    .show_ui(ui, |ui| {
                        for &q in Quality::all() {
                            ui.selectable_value(&mut self.config.quality, q.to_string(), q);
                        }
                    });
                ui.end_row();

                ui.label("API Key:");
                ui.add(egui::TextEdit::singleline(&mut self.config.api_key)
                    .password(true)
                    .desired_width(260.0));
                ui.end_row();
            });

        ui.add_space(4.0);
        ui.colored_label(
            egui::Color32::YELLOW,
            "⚠ APIキーはプレーンテキストで設定ファイルに保存されます",
        );
        ui.add_space(8.0);

        if ui.button("保存").clicked() {
            match self.config.save() {
                Ok(_)  => self.settings_saved_msg = Some("保存しました".to_string()),
                Err(e) => self.settings_saved_msg = Some(format!("保存失敗: {e}")),
            }
        }
        if let Some(msg) = &self.settings_saved_msg {
            ui.label(msg);
        }
    }
}

impl eframe::App for SdsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll for completion
        if self.is_busy() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        // Drain async validate results into val_results
        if let Ok(mut slot) = self.val_pending.try_lock() {
            if let Some(results) = slot.take() {
                self.val_results = results;
            }
        }

        // Tab bar
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Convert,  "変換 (to-json)");
                ui.selectable_value(&mut self.tab, Tab::Generate, "生成 (docx/html)");
                ui.selectable_value(&mut self.tab, Tab::Validate, "検証 (validate)");
                ui.selectable_value(&mut self.tab, Tab::Settings, "設定");
            });
        });

        // Log panel at bottom
        egui::TopBottomPanel::bottom("log_panel").resizable(true).min_height(80.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("ログ");
                if ui.small_button("クリア").clicked() {
                    if let Ok(mut v) = self.log.lock() {
                        v.clear();
                    }
                }
            });
            ui.separator();
            let log_snapshot = self.log.lock().map(|v| v.clone()).unwrap_or_default();
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .max_height(200.0)
                .show(ui, |ui| {
                    for line in &log_snapshot {
                        let color = if line.starts_with("[ERROR]") {
                            egui::Color32::RED
                        } else if line.starts_with("WARN") {
                            egui::Color32::YELLOW
                        } else if line.starts_with("[OK]") || line.starts_with("Saved") {
                            egui::Color32::GREEN
                        } else {
                            ui.visuals().text_color()
                        };
                        ui.colored_label(color, line);
                    }
                });
        });

        // Main content
        egui::CentralPanel::default().show(ctx, |ui| {
            // Need to borrow ctx separately for the sub-functions
            match self.tab {
                Tab::Convert  => {
                    let ctx2 = ctx.clone();
                    self.ui_convert_tab(ui, &ctx2);
                }
                Tab::Generate => {
                    let ctx2 = ctx.clone();
                    self.ui_generate_tab(ui, &ctx2);
                }
                Tab::Validate => {
                    let ctx2 = ctx.clone();
                    self.ui_validate_tab(ui, &ctx2);
                }
                Tab::Settings => self.ui_settings_tab(ui),
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn lang_combo(ui: &mut egui::Ui, id: &str, value: &mut String) {
    let langs = [("ja", "日本語"), ("en", "English"), ("zh-cn", "简体中文"), ("zh-tw", "繁體中文")];
    let label = langs.iter().find(|(k, _)| k == value).map(|(_, v)| *v).unwrap_or("ja");
    egui::ComboBox::from_id_salt(id)
        .selected_text(label)
        .width(110.0)
        .show_ui(ui, |ui| {
            for (k, v) in langs {
                ui.selectable_value(value, k.to_string(), v);
            }
        });
}

fn lang_from_str(s: &str) -> Option<sds_converter_core::Language> {
    match s {
        "ja"    => Some(sds_converter_core::Language::Japanese),
        "en"    => Some(sds_converter_core::Language::English),
        "zh-cn" => Some(sds_converter_core::Language::ChineseSimplified),
        "zh-tw" => Some(sds_converter_core::Language::ChineseTraditional),
        _       => None,
    }
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    #[cfg(target_os = "macos")]
    let candidates: &[&str] = &[
        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
        "/System/Library/Fonts/ヒラギノ角ゴシック W4.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
    ];
    #[cfg(target_os = "windows")]
    let candidates: &[&str] = &[
        "C:/Windows/Fonts/meiryo.ttc",
        "C:/Windows/Fonts/YuGothM.ttc",
        "C:/Windows/Fonts/msgothic.ttc",
    ];
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let candidates: &[&str] = &[
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJKjp-Regular.otf",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf",
    ];

    for path in candidates {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert(
                "jp_font".to_owned(),
                egui::FontData::from_owned(data),
            );
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(family).or_default().push("jp_font".to_owned());
            }
            break;
        }
    }

    ctx.set_fonts(fonts);
}

pub fn run_gui() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("sds-converter")
            .with_inner_size([720.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "sds-converter",
        options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(SdsApp::new()))
        }),
    )
    .map_err(|e| anyhow::anyhow!("GUI error: {e}"))
}

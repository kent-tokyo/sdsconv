use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;

use crate::config::AppConfig;
use crate::tasks::{
    ExtractTextParams, LogFn, Provider, Quality, ToDocxParams, ToHtmlParams, ToJsonParams, ToPdfParams,
};

// ---------------------------------------------------------------------------
// i18n string table
// ---------------------------------------------------------------------------

struct Strings {
    // Menus
    menu_file: &'static str,
    menu_quit: &'static str,
    menu_help: &'static str,
    menu_about: &'static str,
    // Tabs
    tab_convert: &'static str,
    tab_generate: &'static str,
    tab_validate: &'static str,
    tab_settings: &'static str,
    // Convert tab
    heading_convert: &'static str,
    lbl_input: &'static str,
    lbl_output_json: &'static str,
    lbl_provider: &'static str,
    lbl_quality: &'static str,
    lbl_lang: &'static str,
    lbl_enrich: &'static str,
    lbl_files: &'static str,
    btn_browse: &'static str,
    btn_browse_multi: &'static str,
    btn_browse_dir: &'static str,
    btn_save_to: &'static str,
    btn_convert: &'static str,
    btn_converting: &'static str,
    btn_clear_files: &'static str,
    btn_switch_single: &'static str,
    lbl_output_dir: &'static str,
    // Generate tab
    heading_generate: &'static str,
    lbl_input_json: &'static str,
    lbl_output_file: &'static str,
    lbl_format: &'static str,
    btn_generate: &'static str,
    btn_generating: &'static str,
    // Validate tab
    heading_validate: &'static str,
    btn_validate: &'static str,
    btn_validating: &'static str,
    msg_no_issues: &'static str,
    lbl_validate_legend: &'static str,
    // Settings tab
    heading_settings: &'static str,
    lbl_def_provider: &'static str,
    lbl_def_lang: &'static str,
    lbl_def_quality: &'static str,
    lbl_api_key: &'static str,
    lbl_ui_lang: &'static str,
    lbl_def_enrich: &'static str,
    btn_save: &'static str,
    msg_api_key_warn: &'static str,
    msg_saved: &'static str,
    lbl_get_api_key: &'static str,
    lbl_recommended: &'static str,
    banner_no_api_key: &'static str,
    // Log panel
    lbl_log: &'static str,
    btn_clear: &'static str,
    // Errors / status
    err_no_api_key: &'static str,
    err_no_input: &'static str,
    err_create_dir: &'static str,
    msg_start: &'static str,
    msg_done_batch: &'static str,
    // About / Manual
    about_title: &'static str,
    about_desc: &'static str,
    menu_manual: &'static str,
    manual_title: &'static str,
    manual_body: &'static str,
    // Quality tooltips
    tooltip_quality_low: &'static str,
    tooltip_quality_med: &'static str,
    tooltip_quality_high: &'static str,
    // Settings — advanced LLM fields
    lbl_model: &'static str,
    lbl_base_url: &'static str,
    // Generate tab — template
    lbl_template: &'static str,
    // Extract tab
    tab_extract: &'static str,
    heading_extract: &'static str,
    lbl_extract_input: &'static str,
    lbl_extract_output: &'static str,
    btn_extract: &'static str,
    btn_extracting: &'static str,
    lbl_extract_result: &'static str,
    // Drag & drop
    msg_drop_files: &'static str,
    // Welcome screen
    welcome_subtitle: &'static str,
    welcome_btn_convert_title: &'static str,
    welcome_btn_convert_desc: &'static str,
    welcome_btn_generate_title: &'static str,
    welcome_btn_generate_desc: &'static str,
    welcome_btn_validate_title: &'static str,
    welcome_btn_validate_desc: &'static str,
}

fn get_strings(ui_lang: &str) -> Strings {
    match ui_lang {
        "en" => Strings {
            menu_file:        "File",
            menu_quit:        "Quit",
            menu_help:        "Help",
            menu_about:       "About",
            tab_convert:      "SDS → JSON",
            tab_generate:     "Generate Document",
            tab_validate:     "Validate",
            tab_settings:     "Settings",
            heading_convert:  "SDS Document → MHLW Standard JSON",
            lbl_input:        "Input (file/URL):",
            lbl_output_json:  "Output JSON:",
            lbl_provider:     "Provider:",
            lbl_quality:      "Quality:",
            lbl_lang:         "Language:",
            lbl_enrich:       "PubChem lookup",
            lbl_files:        "file(s) selected",
            btn_browse:       "Browse...",
            btn_browse_multi: "Select files...",
            btn_browse_dir:   "Select folder...",
            btn_save_to:      "Save to...",
            btn_convert:      "Convert",
            btn_converting:   "Converting...",
            btn_clear_files:  "Clear selection",
            btn_switch_single: "Switch to single file",
            lbl_output_dir:   "Output folder:",
            heading_generate: "MHLW JSON → Document",
            lbl_input_json:   "Input JSON:",
            lbl_output_file:  "Output:",
            lbl_format:       "Format:",
            btn_generate:     "Generate",
            btn_generating:   "Generating...",
            heading_validate: "JSON Validation",
            btn_validate:     "Validate",
            btn_validating:   "Validating...",
            msg_no_issues:    "OK: no issues found",
            lbl_validate_legend: "✅ No issues  ⚠ Warning  ❌ Error",
            heading_settings: "Settings",
            lbl_def_provider: "Default Provider:",
            lbl_def_lang:     "Default Language:",
            lbl_def_quality:  "Default Quality:",
            lbl_api_key:      "API Key:",
            lbl_ui_lang:      "UI Language:",
            lbl_def_enrich:   "PubChem lookup by default",
            btn_save:         "Save",
            msg_api_key_warn: "⚠ API key is stored in plain text",
            msg_saved:        "Saved",
            lbl_get_api_key:  "Get API key:",
            lbl_recommended:  "recommended",
            banner_no_api_key: "No API key set — go to Settings to enter your key.",
            lbl_log:          "Log",
            btn_clear:        "Clear",
            err_no_api_key:   "[ERROR] API key not set. Enter it in Settings.",
            err_no_input:     "[ERROR] Please specify an input file.",
            err_create_dir:   "Failed to create output folder",
            msg_start:        "▶ Start",
            msg_done_batch:   "✓ Done",
            about_title:      "About sds-converter",
            about_desc:       "Converts SDS documents to/from MHLW standard JSON",
            menu_manual: "Manual",
            manual_title: "How to use sds-converter",
            manual_body: "\
【Convert tab (to-json)】
Convert SDS documents (PDF, Word, XLSX, HTML) to MHLW standard JSON.
1. Enter the file path or URL in the Input field (or click Browse)
2. Specify the output JSON path
3. Select the LLM provider and enter the API key in Settings
4. Click Convert
For batch processing, use 'Select files...' or 'Select folder...'

【Generate tab (docx/html)】
Generate a Word/HTML/PDF document from MHLW standard JSON.
1. Select the input JSON file
2. Specify the output path and format
3. Click Generate

【Validate tab】
Check if a JSON file conforms to the MHLW SDS standard.
Select a JSON file and click Validate to see any warnings.
Multiple files can be selected at once.

【Settings tab】
• API Key: Enter the key for your chosen LLM provider
• Default Provider/Language/Quality: Used when opening the app
• PubChem lookup: Enriches composition data via PubChem API
• UI Language: Change the interface language

【Tips】
• Set RUST_LOG=info for verbose CLI logging
• Use --help for CLI usage: sds-converter --help",
            tooltip_quality_low:  "Low accuracy, fast & cheap (Haiku)",
            tooltip_quality_med:  "Standard accuracy & speed (Haiku)",
            tooltip_quality_high: "High accuracy, slow & costly (Sonnet)",
            lbl_model:            "Model (optional):",
            lbl_base_url:         "Base URL (optional):",
            lbl_template:         "Template (optional):",
            tab_extract:          "Extract Text",
            heading_extract:      "Extract Raw Text from Document",
            lbl_extract_input:    "Input (file/URL):",
            lbl_extract_output:   "Save to (optional):",
            btn_extract:          "Extract",
            btn_extracting:       "Extracting...",
            lbl_extract_result:   "Extracted text:",
            msg_drop_files:            "Drop files here",
            welcome_subtitle:          "Convert SDS documents to/from MHLW standard JSON",
            welcome_btn_convert_title: "SDS → JSON",
            welcome_btn_convert_desc:  "Convert PDF / Word / URL to standard JSON",
            welcome_btn_generate_title: "Generate Document",
            welcome_btn_generate_desc: "Export JSON as DOCX / HTML / PDF",
            welcome_btn_validate_title: "Validate JSON",
            welcome_btn_validate_desc: "Check conformance to MHLW standard",
        },
        "zh-cn" => Strings {
            menu_file:        "文件",
            menu_quit:        "退出",
            menu_help:        "帮助",
            menu_about:       "关于",
            tab_convert:      "SDS → JSON 转换",
            tab_generate:     "生成文档",
            tab_validate:     "验证",
            tab_settings:     "设置",
            heading_convert:  "SDS文档 → MHLW标准JSON",
            lbl_input:        "输入 (文件/URL):",
            lbl_output_json:  "输出 JSON:",
            lbl_provider:     "提供商:",
            lbl_quality:      "质量:",
            lbl_lang:         "语言:",
            lbl_enrich:       "PubChem查询",
            lbl_files:        "个文件已选择",
            btn_browse:       "浏览...",
            btn_browse_multi: "选择文件...",
            btn_browse_dir:   "选择文件夹...",
            btn_save_to:      "保存到...",
            btn_convert:      "开始转换",
            btn_converting:   "转换中...",
            btn_clear_files:  "清除选择",
            btn_switch_single: "切换单文件",
            lbl_output_dir:   "输出文件夹:",
            heading_generate: "MHLW JSON → 文档生成",
            lbl_input_json:   "输入 JSON:",
            lbl_output_file:  "输出文件:",
            lbl_format:       "格式:",
            btn_generate:     "开始生成",
            btn_generating:   "生成中...",
            heading_validate: "JSON验证",
            btn_validate:     "验证",
            btn_validating:   "验证中...",
            msg_no_issues:    "OK: 未发现问题",
            lbl_validate_legend: "✅ 无问题  ⚠ 警告  ❌ 错误",
            heading_settings: "设置",
            lbl_def_provider: "默认提供商:",
            lbl_def_lang:     "默认语言:",
            lbl_def_quality:  "默认质量:",
            lbl_api_key:      "API密钥:",
            lbl_ui_lang:      "界面语言:",
            lbl_def_enrich:   "默认启用PubChem查询",
            btn_save:         "保存",
            msg_api_key_warn: "⚠ API密钥以明文保存",
            msg_saved:        "已保存",
            lbl_get_api_key:  "获取API密钥:",
            lbl_recommended:  "推荐",
            banner_no_api_key: "未设置API密钥 — 请前往设置页面输入密钥。",
            lbl_log:          "日志",
            btn_clear:        "清除",
            err_no_api_key:   "[ERROR] 未设置API密钥，请在设置中输入。",
            err_no_input:     "[ERROR] 请指定输入文件。",
            err_create_dir:   "无法创建输出文件夹",
            msg_start:        "▶ 开始",
            msg_done_batch:   "✓ 完成",
            about_title:      "关于 sds-converter",
            about_desc:       "将SDS文档转换为MHLW标准JSON的工具",
            menu_manual: "使用手册",
            manual_title: "sds-converter 使用说明",
            manual_body: "\
【转换标签 (to-json)】
将SDS文档（PDF、Word、XLSX、HTML）转换为MHLW标准JSON。
1. 在输入栏输入文件路径或URL（或点击浏览）
2. 指定输出JSON路径
3. 在设置中选择LLM提供商并输入API密钥
4. 点击「开始转换」
批量处理：使用「选择文件...」或「选择文件夹...」

【生成标签 (docx/html)】
从MHLW标准JSON生成Word/HTML/PDF文档。
1. 选择输入JSON文件
2. 指定输出路径和格式
3. 点击「开始生成」

【验证标签】
检查JSON文件是否符合MHLW SDS标准。
选择JSON文件并点击「验证」查看警告。

【设置标签】
• API密钥：输入所选LLM提供商的密钥
• 默认提供商/语言/质量：启动时的默认值
• PubChem查询：通过PubChem API丰富成分数据

【提示】
• 设置 RUST_LOG=info 可查看详细日志（CUI模式）
• CLI用法: sds-converter --help",
            tooltip_quality_low:  "低精度·快速·低成本 (Haiku)",
            tooltip_quality_med:  "标准精度·标准速度 (Haiku)",
            tooltip_quality_high: "高精度·慢速·高成本 (Sonnet)",
            lbl_model:            "模型名（可选）:",
            lbl_base_url:         "Base URL（可选）:",
            lbl_template:         "模板（可选）:",
            tab_extract:          "文本提取",
            heading_extract:      "从文档中提取原始文本",
            lbl_extract_input:    "输入 (文件/URL):",
            lbl_extract_output:   "保存到（可选）:",
            btn_extract:          "提取",
            btn_extracting:       "提取中...",
            lbl_extract_result:   "提取结果:",
            msg_drop_files:            "拖放文件到此处",
            welcome_subtitle:          "将SDS文档与MHLW标准JSON双向转换",
            welcome_btn_convert_title: "SDS → JSON",
            welcome_btn_convert_desc:  "将PDF / Word / URL转换为标准JSON",
            welcome_btn_generate_title: "生成文档",
            welcome_btn_generate_desc: "将JSON导出为DOCX / HTML / PDF",
            welcome_btn_validate_title: "验证JSON",
            welcome_btn_validate_desc: "检验是否符合MHLW标准",
        },
        _ => Strings {  // Japanese (ja, default)
            menu_file:        "ファイル",
            menu_quit:        "終了",
            menu_help:        "ヘルプ",
            menu_about:       "バージョン情報",
            tab_convert:      "SDS → JSON 変換",
            tab_generate:     "文書生成",
            tab_validate:     "検証",
            tab_settings:     "設定",
            heading_convert:  "SDS文書 → MHLW標準JSON",
            lbl_input:        "入力 (ファイル/URL):",
            lbl_output_json:  "出力 JSON:",
            lbl_provider:     "プロバイダ:",
            lbl_quality:      "品質:",
            lbl_lang:         "言語:",
            lbl_enrich:       "PubChem照合",
            lbl_files:        "ファイル選択済み",
            btn_browse:       "参照...",
            btn_browse_multi: "複数選択...",
            btn_browse_dir:   "フォルダ選択...",
            btn_save_to:      "保存先...",
            btn_convert:      "変換開始",
            btn_converting:   "変換中...",
            btn_clear_files:  "選択解除",
            btn_switch_single: "単一ファイルに切替",
            lbl_output_dir:   "出力フォルダ:",
            heading_generate: "MHLW JSON → 文書生成",
            lbl_input_json:   "入力 JSON:",
            lbl_output_file:  "出力ファイル:",
            lbl_format:       "形式:",
            btn_generate:     "生成開始",
            btn_generating:   "生成中...",
            heading_validate: "JSON バリデーション",
            btn_validate:     "検証実行",
            btn_validating:   "検証中...",
            msg_no_issues:    "OK: 問題は見つかりませんでした",
            lbl_validate_legend: "✅ 問題なし  ⚠ 警告  ❌ エラー",
            heading_settings: "設定",
            lbl_def_provider: "デフォルトプロバイダ:",
            lbl_def_lang:     "デフォルト言語:",
            lbl_def_quality:  "デフォルト品質:",
            lbl_api_key:      "API Key:",
            lbl_ui_lang:      "UI言語:",
            lbl_def_enrich:   "PubChem照合をデフォルトで有効にする",
            btn_save:         "保存",
            msg_api_key_warn: "⚠ APIキーはプレーンテキストで設定ファイルに保存されます",
            msg_saved:        "保存しました",
            lbl_get_api_key:  "APIキーを取得:",
            lbl_recommended:  "推奨",
            banner_no_api_key: "APIキーが未設定です — 設定タブでキーを入力してください。",
            lbl_log:          "ログ",
            btn_clear:        "クリア",
            err_no_api_key:   "[ERROR] APIキーが未設定です。設定タブで入力してください。",
            err_no_input:     "[ERROR] 入力ファイルを指定してください。",
            err_create_dir:   "出力フォルダを作成できませんでした",
            msg_start:        "▶ 開始",
            msg_done_batch:   "✓ 完了",
            about_title:      "sds-converter について",
            about_desc:       "SDS文書をMHLW標準JSONへ変換するツール",
            menu_manual: "マニュアル",
            manual_title: "sds-converter 使い方",
            manual_body: "\
【変換タブ (to-json)】
SDS文書（PDF・Word・XLSX・HTML）をMHLW標準JSONに変換します。
1. 入力欄にファイルパスまたはURLを入力（または「参照...」で選択）
2. 出力JSONの保存先を指定
3. 設定タブでLLMプロバイダとAPIキーを設定
4. 「変換開始」をクリック
複数ファイルをまとめて変換する場合は「複数選択...」または「フォルダ選択...」を使用

【生成タブ (docx/html)】
MHLW標準JSONからWord・HTML・PDF文書を生成します。
1. 入力JSONファイルを選択
2. 出力先と形式（DOCX/HTML/PDF）を指定
3. 「生成開始」をクリック

【検証タブ】
JSONファイルがMHLW SDS標準に準拠しているか確認します。
JSONファイルを選択して「検証実行」をクリックすると警告が表示されます。
複数ファイルの一括検証も可能です。

【設定タブ】
• APIキー: LLMプロバイダのAPIキーを入力
• デフォルトプロバイダ/言語/品質: 起動時のデフォルト値
• PubChem照合: PubChem APIで組成情報を補完
• UI言語: インターフェースの表示言語を切り替え

【ヒント】
• RUST_LOG=info を設定すると詳細ログが表示されます（CUIモード）
• CLIの使い方: sds-converter --help",
            tooltip_quality_low:  "低精度・高速・低コスト (Haiku)",
            tooltip_quality_med:  "標準精度・標準速度 (Haiku)",
            tooltip_quality_high: "高精度・低速・高コスト (Sonnet)",
            lbl_model:            "モデル名 (省略可):",
            lbl_base_url:         "base URL (省略可):",
            lbl_template:         "テンプレート (省略可):",
            tab_extract:          "テキスト抽出",
            heading_extract:      "文書からテキストを抽出",
            lbl_extract_input:    "入力 (ファイル/URL):",
            lbl_extract_output:   "保存先 (省略可):",
            btn_extract:          "テキスト抽出",
            btn_extracting:       "抽出中...",
            lbl_extract_result:   "抽出結果:",
            msg_drop_files:            "ここにドロップ",
            welcome_subtitle:          "SDS文書とMHLW標準JSONを双方向に変換",
            welcome_btn_convert_title: "SDS → JSON 変換",
            welcome_btn_convert_desc:  "PDF・Word・URLを標準JSONに変換",
            welcome_btn_generate_title: "文書生成",
            welcome_btn_generate_desc: "JSONをDOCX / HTML / PDFで出力",
            welcome_btn_validate_title: "JSON 検証",
            welcome_btn_validate_desc: "MHLW標準への適合を確認",
        },
    }
}

// ---------------------------------------------------------------------------
// Tab / format enums
// ---------------------------------------------------------------------------

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Convert,
    Generate,
    Validate,
    Extract,
    Settings,
}

#[derive(PartialEq, Clone, Copy)]
enum GenFormat {
    Docx,
    Html,
    Pdf,
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub struct SdsApp {
    config: AppConfig,
    rt: tokio::runtime::Runtime,
    log: Arc<Mutex<Vec<String>>>,
    busy: Arc<AtomicBool>,
    tab: Tab,
    show_about: bool,
    show_manual: bool,
    error_modal: Option<String>,

    // Convert tab — batch-capable
    conv_input: String,          // URL or single file path (text box)
    conv_inputs: Vec<PathBuf>,   // multi-file selection (non-empty → batch mode)
    conv_output: String,         // single-file output path
    conv_output_dir: String,     // batch output directory
    conv_provider: String,
    conv_quality: String,
    conv_lang: String,
    conv_enrich: bool,

    // Generate tab
    gen_input: String,
    gen_output: String,
    gen_format: GenFormat,
    gen_lang: String,
    gen_template: String,

    // Extract tab
    extract_input: String,
    extract_output: String,
    extract_result: Arc<Mutex<Option<String>>>,
    extract_result_display: String,

    // Validate tab — batch-capable
    val_input: String,
    val_inputs: Vec<PathBuf>,
    val_results: Vec<String>,
    val_pending: Arc<Mutex<Option<Vec<String>>>>,

    // Settings tab
    settings_saved_msg: Option<String>,

    // Welcome screen
    show_welcome: bool,
}

impl SdsApp {
    pub fn new() -> Self {
        let config = AppConfig::load();
        Self {
            conv_provider: config.provider.clone(),
            conv_quality:  config.quality.clone(),
            conv_lang:     config.language.clone(),
            conv_enrich:   config.enrich,
            gen_lang:      config.language.clone(),
            config,
            rt: tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
            log:          Arc::new(Mutex::new(Vec::new())),
            busy:         Arc::new(AtomicBool::new(false)),
            tab:          Tab::Convert,
            show_about:   false,
            show_manual:  false,
            error_modal:  None,
            conv_input:   String::new(),
            conv_inputs:  Vec::new(),
            conv_output:  String::new(),
            conv_output_dir: String::new(),
            gen_input:    String::new(),
            gen_output:   String::new(),
            gen_format:   GenFormat::Docx,
            gen_template: String::new(),
            extract_input:          String::new(),
            extract_output:         String::new(),
            extract_result:         Arc::new(Mutex::new(None)),
            extract_result_display: String::new(),
            val_input:    String::new(),
            val_inputs:   Vec::new(),
            val_results:  Vec::new(),
            val_pending:  Arc::new(Mutex::new(None)),
            settings_saved_msg: None,
            show_welcome: true,
        }
    }

    fn log_push(&self, msg: impl Into<String>) {
        if let Ok(mut v) = self.log.lock() { v.push(msg.into()); }
    }

    fn make_log_fn(&self) -> LogFn {
        let log = Arc::clone(&self.log);
        Arc::new(move |msg| { if let Ok(mut v) = log.lock() { v.push(msg); } })
    }

    fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Relaxed)
    }

    fn s(&self) -> Strings {
        get_strings(&self.config.ui_lang)
    }

    // -----------------------------------------------------------------------
    // Convert tab
    // -----------------------------------------------------------------------

    fn ui_convert_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let s = self.s();
        ui.heading(s.heading_convert);
        ui.add_space(10.0);

        let batch = !self.conv_inputs.is_empty();

        if batch {
            // Batch mode: show file count + switch-to-single + clear button
            ui.horizontal(|ui| {
                ui.label(format!("{} {}", self.conv_inputs.len(), s.lbl_files));
                if ui.button(s.btn_switch_single).clicked() {
                    self.conv_inputs.clear();
                }
                if ui.small_button(s.btn_clear_files).clicked() {
                    self.conv_inputs.clear();
                }
            });
            ui.horizontal(|ui| {
                ui.label(s.lbl_output_dir);
                ui.add_sized([260.0, 20.0], egui::TextEdit::singleline(&mut self.conv_output_dir));
                if ui.button(s.btn_browse_dir).clicked() {
                    if let Some(p) = rfd::FileDialog::new().pick_folder() {
                        self.conv_output_dir = p.to_string_lossy().into_owned();
                    }
                }
            });
        } else {
            // Single mode: text input + browse
            ui.horizontal(|ui| {
                ui.label(s.lbl_input);
                ui.add_sized([280.0, 20.0], egui::TextEdit::singleline(&mut self.conv_input));
                if ui.button(s.btn_browse).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("SDS", &["pdf", "docx", "xlsx", "txt", "html"])
                        .pick_file()
                    {
                        self.conv_input = path.to_string_lossy().into_owned();
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
                ui.label(s.lbl_output_json);
                ui.add_sized([280.0, 20.0], egui::TextEdit::singleline(&mut self.conv_output));
                if ui.button(s.btn_save_to).clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("JSON", &["json"]).save_file() {
                        self.conv_output = p.to_string_lossy().into_owned();
                    }
                }
            });
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button(s.btn_browse_multi).clicked() {
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter("SDS", &["pdf", "docx", "xlsx", "txt", "html"])
                    .pick_files()
                {
                    self.conv_inputs = paths;
                    self.conv_output.clear();
                }
            }
            if ui.button(s.btn_browse_dir).clicked() {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    let exts = ["pdf", "docx", "xlsx", "txt", "html"];
                    self.conv_inputs = crate::tasks::collect_files(&dir, &exts);
                    self.conv_output_dir = dir.to_string_lossy().into_owned();
                }
            }
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(s.lbl_provider);
            egui::ComboBox::from_id_salt("conv_provider")
                .selected_text(&self.conv_provider)
                .width(130.0)
                .show_ui(ui, |ui| {
                    for &p in Provider::all() {
                        ui.selectable_value(&mut self.conv_provider, p.to_string(), p);
                    }
                });
            ui.add_space(8.0);
            ui.label(s.lbl_quality);
            egui::ComboBox::from_id_salt("conv_quality")
                .selected_text(&self.conv_quality)
                .width(85.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.conv_quality, "low".to_string(), "low")
                        .on_hover_text(s.tooltip_quality_low);
                    ui.selectable_value(&mut self.conv_quality, "medium".to_string(), "medium")
                        .on_hover_text(s.tooltip_quality_med);
                    ui.selectable_value(&mut self.conv_quality, "high".to_string(), "high")
                        .on_hover_text(s.tooltip_quality_high);
                });
            ui.add_space(8.0);
            ui.label(s.lbl_lang);
            lang_combo(ui, "conv_lang", &mut self.conv_lang);
        });

        ui.checkbox(&mut self.conv_enrich, s.lbl_enrich);
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            let label = if self.is_busy() { s.btn_converting } else { s.btn_convert };
            if ui.add_enabled(!self.is_busy(), egui::Button::new(label)).clicked() {
                self.start_convert(ctx);
            }
            if self.is_busy() { ui.spinner(); }
        });
    }

    fn start_convert(&mut self, ctx: &egui::Context) {
        let provider = Provider::from_str(&self.conv_provider);
        let quality  = Quality::from_str(&self.conv_quality);
        let lang     = lang_from_str(&self.conv_lang);
        let enrich   = self.conv_enrich;
        let s        = self.s();

        let api_key = {
            let k = self.config.api_key.clone();
            if k.is_empty() {
                std::env::var(provider.api_key_env()).unwrap_or_default()
            } else { k }
        };
        if api_key.is_empty() {
            self.log_push(s.err_no_api_key);
            return;
        }

        let model = if self.config.model.is_empty() {
            provider.default_model(quality).to_string()
        } else {
            self.config.model.clone()
        };
        let base_url = if self.config.base_url.is_empty() { None } else { Some(self.config.base_url.clone()) };
        let log_fn  = self.make_log_fn();
        let log_err = Arc::clone(&self.log);
        let busy    = Arc::clone(&self.busy);
        let ctx2    = ctx.clone();
        busy.store(true, Ordering::Relaxed);

        if !self.conv_inputs.is_empty() {
            // ----- Batch mode -----
            let inputs = self.conv_inputs.clone();
            let out_dir = PathBuf::from(if self.conv_output_dir.is_empty() {
                inputs.first().and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned()))
                    .unwrap_or_default()
            } else {
                self.conv_output_dir.clone()
            });
            let msg_start = s.msg_start.to_string();
            let err_create_dir = s.err_create_dir.to_string();
            let msg_done_batch = s.msg_done_batch.to_string();
            self.log_push(format!("{} batch {} files", msg_start, inputs.len()));

            self.rt.spawn(async move {
                if let Err(e) = std::fs::create_dir_all(&out_dir) {
                    if let Ok(mut v) = log_err.lock() {
                        v.push(format!("[ERROR] {}: {e}", err_create_dir));
                    }
                    busy.store(false, Ordering::Relaxed);
                    ctx2.request_repaint();
                    return;
                }
                let total = inputs.len();
                let mut ok = 0usize;
                for path in &inputs {
                    let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                    let output = out_dir.join(format!("{stem}.json"));
                    let res = crate::tasks::run_to_json(ToJsonParams {
                        input: path.to_string_lossy().into_owned(),
                        output,
                        provider, api_key: api_key.clone(), model: model.clone(),
                        quality, lang, base_url: base_url.clone(), enrich,
                    }, Arc::clone(&log_fn)).await;
                    match res {
                        Ok(_)  => ok += 1,
                        Err(e) => { if let Ok(mut v) = log_err.lock() { v.push(format!("[ERROR] {e}")); } }
                    }
                }
                if let Ok(mut v) = log_err.lock() {
                    v.push(format!("{} {ok}/{total} converted", msg_done_batch));
                }
                busy.store(false, Ordering::Relaxed);
                ctx2.request_repaint();
            });
        } else {
            // ----- Single mode -----
            let input  = self.conv_input.trim().to_string();
            let output = PathBuf::from(self.conv_output.trim());
            if input.is_empty() {
                self.error_modal = Some(s.err_no_input.to_string());
                busy.store(false, Ordering::Relaxed);
                return;
            }
            if output.as_os_str().is_empty() {
                self.log_push("[ERROR] 出力パスを指定してください。");
                busy.store(false, Ordering::Relaxed);
                return;
            }
            let msg_start = s.msg_start.to_string();
            self.log_push(format!("{} {} → {}", msg_start, input, output.display()));

            self.rt.spawn(async move {
                if let Err(e) = crate::tasks::run_to_json(ToJsonParams {
                    input, output, provider, api_key, model, quality, lang, base_url, enrich,
                }, log_fn).await {
                    if let Ok(mut v) = log_err.lock() { v.push(format!("[ERROR] {e}")); }
                }
                busy.store(false, Ordering::Relaxed);
                ctx2.request_repaint();
            });
        }
    }

    // -----------------------------------------------------------------------
    // Generate tab
    // -----------------------------------------------------------------------

    fn ui_generate_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let s = self.s();
        ui.heading(s.heading_generate);
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label(s.lbl_input_json);
            ui.add_sized([280.0, 20.0], egui::TextEdit::singleline(&mut self.gen_input));
            if ui.button(s.btn_browse).clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                    self.gen_input = path.to_string_lossy().into_owned();
                    if self.gen_output.is_empty() {
                        let ext = match self.gen_format {
                            GenFormat::Docx => "docx", GenFormat::Html => "html", GenFormat::Pdf => "pdf",
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
            ui.label(s.lbl_output_file);
            ui.add_sized([280.0, 20.0], egui::TextEdit::singleline(&mut self.gen_output));
            if ui.button(s.btn_save_to).clicked() {
                let (desc, exts): (&str, Vec<&str>) = match self.gen_format {
                    GenFormat::Docx => ("Word", vec!["docx"]),
                    GenFormat::Html => ("HTML", vec!["html"]),
                    GenFormat::Pdf  => ("PDF",  vec!["pdf"]),
                };
                if let Some(p) = rfd::FileDialog::new().add_filter(desc, &exts).save_file() {
                    self.gen_output = p.to_string_lossy().into_owned();
                }
            }
        });

        // Template picker — shown only when DOCX is selected
        if self.gen_format == GenFormat::Docx {
            ui.horizontal(|ui| {
                ui.label(s.lbl_template);
                ui.add_sized([220.0, 20.0], egui::TextEdit::singleline(&mut self.gen_template));
                if ui.button(s.btn_browse).clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("Word", &["docx"]).pick_file() {
                        self.gen_template = p.to_string_lossy().into_owned();
                    }
                }
                if !self.gen_template.is_empty() && ui.small_button("✕").clicked() {
                    self.gen_template.clear();
                }
            });
        }

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(s.lbl_format);
            egui::ComboBox::from_id_salt("gen_format")
                .selected_text(match self.gen_format {
                    GenFormat::Docx => "DOCX",
                    GenFormat::Html => "HTML",
                    GenFormat::Pdf  => "PDF",
                })
                .width(90.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.gen_format, GenFormat::Docx, "DOCX");
                    ui.selectable_value(&mut self.gen_format, GenFormat::Html, "HTML");
                    ui.selectable_value(&mut self.gen_format, GenFormat::Pdf,  "PDF");
                });
            ui.add_space(12.0);
            ui.label(s.lbl_lang);
            lang_combo(ui, "gen_lang", &mut self.gen_lang);
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let label = if self.is_busy() { s.btn_generating } else { s.btn_generate };
            if ui.add_enabled(!self.is_busy(), egui::Button::new(label)).clicked() {
                self.start_generate(ctx);
            }
            if self.is_busy() { ui.spinner(); }
        });
    }

    fn start_generate(&mut self, ctx: &egui::Context) {
        let s = self.s();
        if self.gen_input.is_empty() {
            self.error_modal = Some(s.err_no_input.to_string());
            return;
        }
        let input    = PathBuf::from(self.gen_input.trim());
        let output   = PathBuf::from(self.gen_output.trim());
        let lang     = lang_from_str(&self.gen_lang).unwrap_or(sds_converter_core::Language::Japanese);
        let format   = self.gen_format;
        let template = if self.gen_template.is_empty() { None }
                       else { Some(PathBuf::from(self.gen_template.trim())) };

        let log_fn  = self.make_log_fn();
        let log_err = Arc::clone(&self.log);
        let busy    = Arc::clone(&self.busy);
        let ctx2    = ctx.clone();
        busy.store(true, Ordering::Relaxed);
        let msg_start = s.msg_start.to_string();
        self.log_push(format!("{} {} → {}", msg_start, input.display(), output.display()));

        self.rt.spawn(async move {
            let result = match format {
                GenFormat::Docx => crate::tasks::run_to_docx(
                    ToDocxParams { input, output, lang, template }, log_fn).await,
                GenFormat::Html => crate::tasks::run_to_html(
                    ToHtmlParams { input, output, lang }, log_fn).await,
                GenFormat::Pdf  => crate::tasks::run_to_pdf(
                    ToPdfParams { input, output, lang }, log_fn).await,
            };
            if let Err(e) = result {
                if let Ok(mut v) = log_err.lock() { v.push(format!("[ERROR] {e}")); }
            }
            busy.store(false, Ordering::Relaxed);
            ctx2.request_repaint();
        });
    }

    // -----------------------------------------------------------------------
    // Validate tab
    // -----------------------------------------------------------------------

    fn ui_validate_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let s = self.s();
        ui.heading(s.heading_validate);
        ui.add_space(10.0);

        let batch = !self.val_inputs.is_empty();

        if batch {
            ui.horizontal(|ui| {
                ui.label(format!("{} {}", self.val_inputs.len(), s.lbl_files));
                if ui.button(s.btn_switch_single).clicked() {
                    self.val_inputs.clear();
                    self.val_results.clear();
                }
                if ui.small_button(s.btn_clear_files).clicked() {
                    self.val_inputs.clear();
                    self.val_results.clear();
                }
            });
        } else {
            ui.horizontal(|ui| {
                ui.label(s.lbl_input_json);
                ui.add_sized([280.0, 20.0], egui::TextEdit::singleline(&mut self.val_input));
                if ui.button(s.btn_browse).clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                        self.val_input = path.to_string_lossy().into_owned();
                        self.val_results.clear();
                    }
                }
            });
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button(s.btn_browse_multi).clicked() {
                if let Some(paths) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_files() {
                    self.val_inputs = paths;
                    self.val_results.clear();
                }
            }
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let label = if self.is_busy() { s.btn_validating } else { s.btn_validate };
            if ui.add_enabled(!self.is_busy(), egui::Button::new(label)).clicked() {
                self.start_validate(ctx);
            }
            if self.is_busy() { ui.spinner(); }
        });

        if !self.val_results.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            ui.small(s.lbl_validate_legend);
            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                for w in &self.val_results {
                    let color = if w.starts_with("✅") {
                        egui::Color32::GREEN
                    } else if w.starts_with("❌") || w.starts_with("[ERROR]") {
                        egui::Color32::RED
                    } else {
                        egui::Color32::YELLOW
                    };
                    ui.colored_label(color, w);
                }
            });
        }
    }

    fn start_validate(&mut self, ctx: &egui::Context) {
        self.val_results.clear();
        if let Ok(mut slot) = self.val_pending.lock() { *slot = None; }
        let log_fn  = self.make_log_fn();
        let busy    = Arc::clone(&self.busy);
        let ctx2    = ctx.clone();
        let pending = Arc::clone(&self.val_pending);
        busy.store(true, Ordering::Relaxed);
        let s = self.s();

        let inputs: Vec<PathBuf> = if !self.val_inputs.is_empty() {
            self.val_inputs.clone()
        } else if !self.val_input.is_empty() {
            vec![PathBuf::from(self.val_input.trim())]
        } else {
            self.log_push(s.err_no_input);
            busy.store(false, Ordering::Relaxed);
            return;
        };

        let ok_prefix  = "✅ ".to_string();
        let warn_prefix = "⚠ ".to_string();
        let err_prefix  = "❌ ".to_string();

        self.rt.spawn(async move {
            let mut all_results: Vec<String> = Vec::new();
            for path in &inputs {
                let prefix = if inputs.len() > 1 {
                    format!("[{}] ", path.file_name().unwrap_or_default().to_string_lossy())
                } else {
                    String::new()
                };
                match crate::tasks::run_validate(path.clone(), Arc::clone(&log_fn)).await {
                    Ok(warnings) if warnings.is_empty() => {
                        all_results.push(format!("{}{}問題なし", prefix, ok_prefix));
                    }
                    Ok(warnings) => {
                        for w in warnings { all_results.push(format!("{prefix}{warn_prefix}{w}")); }
                    }
                    Err(e) => {
                        all_results.push(format!("{prefix}{err_prefix}{e}"));
                    }
                }
            }
            if let Ok(mut slot) = pending.lock() { *slot = Some(all_results); }
            busy.store(false, Ordering::Relaxed);
            ctx2.request_repaint();
        });
    }

    // -----------------------------------------------------------------------
    // Extract tab
    // -----------------------------------------------------------------------

    fn ui_extract_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let s = self.s();
        ui.heading(s.heading_extract);
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label(s.lbl_extract_input);
            ui.add_sized([280.0, 20.0], egui::TextEdit::singleline(&mut self.extract_input));
            if ui.button(s.btn_browse).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Document", &["pdf", "docx", "xlsx", "txt", "html"])
                    .pick_file()
                {
                    self.extract_input = path.to_string_lossy().into_owned();
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label(s.lbl_extract_output);
            ui.add_sized([280.0, 20.0], egui::TextEdit::singleline(&mut self.extract_output));
            if ui.button(s.btn_save_to).clicked() {
                if let Some(p) = rfd::FileDialog::new().add_filter("Text", &["txt"]).save_file() {
                    self.extract_output = p.to_string_lossy().into_owned();
                }
            }
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let label = if self.is_busy() { s.btn_extracting } else { s.btn_extract };
            if ui.add_enabled(!self.is_busy(), egui::Button::new(label)).clicked() {
                self.start_extract(ctx);
            }
            if self.is_busy() { ui.spinner(); }
        });

        if !self.extract_result_display.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            ui.label(s.lbl_extract_result);
            egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut self.extract_result_display)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace));
            });
        }
    }

    fn start_extract(&mut self, ctx: &egui::Context) {
        let s = self.s();
        if self.extract_input.is_empty() {
            self.error_modal = Some(s.err_no_input.to_string());
            return;
        }
        let params = ExtractTextParams {
            input: self.extract_input.trim().to_string(),
            output: if self.extract_output.is_empty() { None }
                    else { Some(PathBuf::from(self.extract_output.trim())) },
        };
        let log_fn  = self.make_log_fn();
        let log_err = Arc::clone(&self.log);
        let busy    = Arc::clone(&self.busy);
        let ctx2    = ctx.clone();
        let result_sink = Arc::clone(&self.extract_result);
        busy.store(true, Ordering::Relaxed);

        self.rt.spawn(async move {
            match crate::tasks::run_extract_text(params, log_fn).await {
                Ok(text) => {
                    if let Ok(mut slot) = result_sink.lock() { *slot = Some(text); }
                }
                Err(e) => {
                    if let Ok(mut v) = log_err.lock() { v.push(format!("[ERROR] {e}")); }
                }
            }
            busy.store(false, Ordering::Relaxed);
            ctx2.request_repaint();
        });
    }

    // -----------------------------------------------------------------------
    // Settings tab
    // -----------------------------------------------------------------------

    fn ui_settings_tab(&mut self, ui: &mut egui::Ui) {
        let s = self.s();

        // B6: onboarding banner when no key is saved
        if self.config.api_key.is_empty() {
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(55, 45, 0))
                .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                .rounding(egui::Rounding::same(4.0))
                .show(ui, |ui| {
                    ui.colored_label(egui::Color32::from_rgb(255, 220, 60), s.banner_no_api_key);
                });
            ui.add_space(4.0);
        }

        ui.heading(s.heading_settings);
        ui.add_space(10.0);

        egui::Grid::new("settings_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label(s.lbl_def_provider);
            egui::ComboBox::from_id_salt("settings_provider")
                .selected_text(&self.config.provider)
                .width(130.0)
                .show_ui(ui, |ui| {
                    for &p in Provider::all() {
                        let label = if p == "anthropic" {
                            format!("{p} ({})", s.lbl_recommended)
                        } else {
                            p.to_string()
                        };
                        ui.selectable_value(&mut self.config.provider, p.to_string(), label);
                    }
                });
            ui.end_row();

            ui.label(s.lbl_model);
            ui.add(egui::TextEdit::singleline(&mut self.config.model).desired_width(240.0));
            ui.end_row();

            ui.label(s.lbl_base_url);
            ui.add(egui::TextEdit::singleline(&mut self.config.base_url).desired_width(240.0));
            ui.end_row();

            ui.label(s.lbl_def_lang);
            lang_combo(ui, "settings_lang", &mut self.config.language);
            ui.end_row();

            ui.label(s.lbl_def_quality);
            egui::ComboBox::from_id_salt("settings_quality")
                .selected_text(&self.config.quality)
                .width(85.0)
                .show_ui(ui, |ui| {
                    for &q in Quality::all() {
                        ui.selectable_value(&mut self.config.quality, q.to_string(), q);
                    }
                });
            ui.end_row();

            ui.label(s.lbl_api_key);
            ui.add(egui::TextEdit::singleline(&mut self.config.api_key)
                .password(true).desired_width(240.0));
            ui.end_row();

            // B4: API key link per provider
            ui.label(s.lbl_get_api_key);
            let link = match self.config.provider.as_str() {
                "openai"  => Some(("OpenAI Platform", "https://platform.openai.com/api-keys")),
                "gemini"  => Some(("Google AI Studio", "https://aistudio.google.com/app/apikey")),
                "mistral" => Some(("Mistral Console",  "https://console.mistral.ai/api-keys/")),
                "groq"    => Some(("Groq Console",     "https://console.groq.com/keys")),
                "cohere"  => Some(("Cohere Dashboard", "https://dashboard.cohere.com/api-keys")),
                "local"   => None,
                _         => Some(("Anthropic Console", "https://console.anthropic.com/settings/keys")),
            };
            if let Some((label, url)) = link {
                ui.hyperlink_to(label, url);
            } else {
                ui.label("(local server — no key required)");
            }
            ui.end_row();

            ui.label(s.lbl_ui_lang);
            let ui_langs = [("ja", "日本語"), ("en", "English"), ("zh-cn", "简体中文")];
            let cur_label = ui_langs.iter().find(|(k, _)| *k == self.config.ui_lang.as_str())
                .map(|(_, v)| *v).unwrap_or("日本語");
            egui::ComboBox::from_id_salt("ui_lang")
                .selected_text(cur_label)
                .width(120.0)
                .show_ui(ui, |ui| {
                    for (k, v) in ui_langs {
                        ui.selectable_value(&mut self.config.ui_lang, k.to_string(), v);
                    }
                });
            ui.end_row();

            ui.label(s.lbl_def_enrich);
            ui.checkbox(&mut self.config.enrich, "");
            ui.end_row();
        });

        ui.add_space(4.0);
        // B5: show warning + config file path
        ui.colored_label(egui::Color32::YELLOW, s.msg_api_key_warn);
        if let Some(path) = crate::config::AppConfig::config_path_pub() {
            ui.small(path.to_string_lossy().as_ref());
        }
        ui.add_space(8.0);

        if ui.button(s.btn_save).clicked() {
            match self.config.save() {
                Ok(_)  => {
                    self.conv_enrich = self.config.enrich;
                    self.settings_saved_msg = Some(s.msg_saved.to_string());
                }
                Err(e) => self.settings_saved_msg = Some(format!("Error: {e}")),
            }
        }
        if let Some(msg) = &self.settings_saved_msg {
            ui.label(msg);
        }
    }

    // -----------------------------------------------------------------------
    // Welcome screen
    // -----------------------------------------------------------------------

    fn ui_welcome_screen(&mut self, ui: &mut egui::Ui) {
        let s = self.s();

        // Vertical centering
        let available_height = ui.available_height();
        let content_height = 320.0;
        ui.add_space(((available_height - content_height) / 2.0).max(24.0));

        ui.vertical_centered(|ui| {
            // App logo — colored rounded rect with "SDS" label
            let (logo_rect, _) = ui.allocate_exact_size(
                egui::vec2(72.0, 72.0),
                egui::Sense::hover(),
            );
            ui.painter().rect_filled(
                logo_rect,
                egui::Rounding::same(14.0),
                egui::Color32::from_rgb(56, 120, 200),
            );
            ui.painter().text(
                logo_rect.center(),
                egui::Align2::CENTER_CENTER,
                "SDS",
                egui::FontId::proportional(24.0),
                egui::Color32::WHITE,
            );

            ui.add_space(14.0);
            ui.label(egui::RichText::new("SDS Converter").size(30.0).strong());
            ui.label(
                egui::RichText::new(concat!("v", env!("CARGO_PKG_VERSION")))
                    .size(13.0)
                    .color(egui::Color32::GRAY),
            );
            ui.add_space(8.0);
            ui.label(egui::RichText::new(s.welcome_subtitle).size(14.0));
            ui.add_space(36.0);

            // Action cards — explicitly centered as a group
            let card_w = 210.0_f32;
            let card_h = 90.0_f32;
            let gap = 12.0_f32;
            let group_w = 3.0 * card_w + 2.0 * gap;
            let left_pad = ((ui.available_width() - group_w) / 2.0).max(0.0);

            ui.horizontal(|ui| {
                ui.add_space(left_pad);

                let card_size = egui::vec2(card_w, card_h);
                let entries: &[(Tab, &str, &str, &str)] = &[
                    (Tab::Convert,  "📄", s.welcome_btn_convert_title,  s.welcome_btn_convert_desc),
                    (Tab::Generate, "📝", s.welcome_btn_generate_title, s.welcome_btn_generate_desc),
                    (Tab::Validate, "✅", s.welcome_btn_validate_title, s.welcome_btn_validate_desc),
                ];

                for (i, &(tab, icon, title, desc)) in entries.iter().enumerate() {
                    if i > 0 { ui.add_space(gap); }

                    let (rect, resp) = ui.allocate_exact_size(card_size, egui::Sense::click());
                    let visuals = ui.style().interact(&resp);
                    ui.painter().rect(
                        rect,
                        egui::Rounding::same(6.0),
                        visuals.bg_fill,
                        visuals.bg_stroke,
                    );

                    // Icon + title (bold, centered)
                    let icon_title = format!("{}  {}", icon, title);
                    ui.painter().text(
                        egui::pos2(rect.center().x, rect.center().y - 14.0),
                        egui::Align2::CENTER_CENTER,
                        &icon_title,
                        egui::FontId::proportional(14.0),
                        visuals.text_color(),
                    );
                    // Description (smaller, gray, centered)
                    ui.painter().text(
                        egui::pos2(rect.center().x, rect.center().y + 14.0),
                        egui::Align2::CENTER_CENTER,
                        desc,
                        egui::FontId::proportional(11.5),
                        egui::Color32::GRAY,
                    );

                    if resp.clicked() {
                        self.tab = tab;
                        self.show_welcome = false;
                    }
                }
            });

            ui.add_space(28.0);
            if ui.small_button("→ skip").clicked() {
                self.show_welcome = false;
            }
        });
    }
}

// ---------------------------------------------------------------------------
// eframe::App impl
// ---------------------------------------------------------------------------

impl eframe::App for SdsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Repaint while busy
        if self.is_busy() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        // Drain async validate results
        if let Ok(mut slot) = self.val_pending.try_lock() {
            if let Some(results) = slot.take() {
                self.val_results = results;
            }
        }

        // Drain async extract results
        if let Ok(mut slot) = self.extract_result.try_lock() {
            if let Some(text) = slot.take() {
                self.extract_result_display = if text.len() > 50_000 {
                    format!("{}\n...(truncated)", &text[..50_000])
                } else {
                    text
                };
            }
        }

        let s = self.s();

        // B11: Escape key closes modals
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.error_modal.is_some() { self.error_modal = None; }
            else if self.show_manual { self.show_manual = false; }
            else if self.show_about { self.show_about = false; }
        }

        // --- Drag & drop ---
        let hovered = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if hovered {
            egui::Area::new(egui::Id::new("drop_overlay"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    let screen = ctx.screen_rect();
                    ui.painter().rect_filled(screen, 0.0, egui::Color32::from_black_alpha(120));
                    ui.painter().text(
                        screen.center(),
                        egui::Align2::CENTER_CENTER,
                        s.msg_drop_files,
                        egui::FontId::proportional(32.0),
                        egui::Color32::WHITE,
                    );
                });
        }
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect()
        });
        if !dropped.is_empty() {
            if self.show_welcome {
                self.show_welcome = false;
                // tab stays Convert — drop routing below handles placement
            }
            match self.tab {
                Tab::Convert => {
                    if dropped.len() == 1 && self.conv_inputs.is_empty() {
                        self.conv_input = dropped[0].to_string_lossy().into_owned();
                    } else {
                        self.conv_inputs.extend(dropped);
                    }
                }
                Tab::Generate => {
                    if let Some(p) = dropped.first() {
                        self.gen_input = p.to_string_lossy().into_owned();
                    }
                }
                Tab::Validate => {
                    self.val_inputs.extend(dropped);
                }
                Tab::Extract => {
                    if let Some(p) = dropped.first() {
                        self.extract_input = p.to_string_lossy().into_owned();
                    }
                }
                Tab::Settings => {}
            }
        }

        // --- Menu bar ---
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button(s.menu_file, |ui| {
                    if ui.button(s.menu_quit).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button(s.menu_help, |ui| {
                    if ui.button(s.menu_manual).clicked() {
                        self.show_manual = true;
                        ui.close_menu();
                    }
                    if ui.button(s.menu_about).clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });

        // --- Tab bar (hidden on welcome screen) ---
        if !self.show_welcome {
            egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, Tab::Convert,  s.tab_convert);
                    ui.selectable_value(&mut self.tab, Tab::Generate, s.tab_generate);
                    ui.selectable_value(&mut self.tab, Tab::Validate, s.tab_validate);
                    ui.selectable_value(&mut self.tab, Tab::Extract,  s.tab_extract);
                    ui.selectable_value(&mut self.tab, Tab::Settings, s.tab_settings);
                });
            });
        }

        // --- Log panel (hidden on welcome screen) ---
        if !self.show_welcome {
        egui::TopBottomPanel::bottom("log_panel").resizable(true).min_height(60.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                // B13: show max-500 note
                ui.label(format!("{} (max 500)", s.lbl_log));
                if ui.small_button(s.btn_clear).clicked() {
                    if let Ok(mut v) = self.log.lock() { v.clear(); }
                }
            });
            ui.separator();
            let lines = self.log.lock().map(|v| v.clone()).unwrap_or_default();
            egui::ScrollArea::vertical().stick_to_bottom(true).max_height(160.0).show(ui, |ui| {
                for line in &lines {
                    let color = if line.starts_with("[ERROR]") { egui::Color32::RED }
                        else if line.starts_with("WARN") || line.starts_with("CAS:") { egui::Color32::YELLOW }
                        else if line.starts_with("[OK]") || line.starts_with("Saved") || line.starts_with("OK") || line.starts_with("[DONE]") || line.starts_with("✓") { egui::Color32::GREEN }
                        else { ui.visuals().text_color() };
                    ui.colored_label(color, line);
                }
            });
        });
        } // end if !self.show_welcome (log panel)

        // --- Main content (with inner margin for breathing room) ---
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(ctx.style().as_ref()).inner_margin(egui::Margin::symmetric(14.0, 10.0)))
            .show(ctx, |ui| {
            if self.show_welcome {
                self.ui_welcome_screen(ui);
            } else {
                let ctx2 = ctx.clone();
                match self.tab {
                    Tab::Convert  => self.ui_convert_tab(ui, &ctx2),
                    Tab::Generate => self.ui_generate_tab(ui, &ctx2),
                    Tab::Validate => self.ui_validate_tab(ui, &ctx2),
                    Tab::Extract  => self.ui_extract_tab(ui, &ctx2),
                    Tab::Settings => self.ui_settings_tab(ui),
                }
            }
        });

        // --- Error modal (B1, B11) ---
        if let Some(ref msg) = self.error_modal.clone() {
            egui::Window::new("Error")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(msg.as_str());
                    ui.add_space(8.0);
                    if ui.button("OK").clicked() {
                        self.error_modal = None;
                    }
                });
        }

        // --- About dialog (B14) ---
        if self.show_about {
            let s = self.s();
            egui::Window::new(s.about_title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(concat!("sds-converter v", env!("CARGO_PKG_VERSION")));
                    ui.add_space(4.0);
                    ui.label(s.about_desc);
                    ui.add_space(4.0);
                    ui.hyperlink_to("GitHub", "https://github.com/kent-tokyo/sds-converter");
                    ui.add_space(8.0);
                    if ui.button("OK").clicked() { self.show_about = false; }
                });
        }

        // --- Manual window ---
        if self.show_manual {
            let s = self.s();
            egui::Window::new(s.manual_title)
                .collapsible(false)
                .resizable(true)
                .default_size([520.0, 420.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.label(s.manual_body);
                    });
                    ui.add_space(8.0);
                    if ui.button("OK").clicked() { self.show_manual = false; }
                });
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn lang_combo(ui: &mut egui::Ui, id: &str, value: &mut String) {
    let langs = [("ja", "日本語"), ("en", "English"), ("zh-cn", "简体中文"), ("zh-tw", "繁體中文")];
    let label = langs.iter().find(|(k, _)| *k == value.as_str()).map(|(_, v)| *v).unwrap_or("日本語");
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

// ---------------------------------------------------------------------------
// Font + launch
// ---------------------------------------------------------------------------

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    #[cfg(target_os = "macos")]
    let candidates: &[&str] = &[
        "/System/Library/Fonts/ヒラギノ角ゴシック W4.ttc",  // W4 preferred: slightly heavier, renders cleaner at small sizes
        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
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
            fonts.font_data.insert("jp_font".to_owned(), egui::FontData::from_owned(data));
            // Primary font: insert at position 0 so Latin and CJK share baseline metrics
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(family).or_default().insert(0, "jp_font".to_owned());
            }
            break;
        }
    }

    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();

    // Bump body/button text from egui's default 14 pt to 15 pt
    for font_id in style.text_styles.values_mut() {
        if (font_id.size - 14.0).abs() < 0.5 {
            font_id.size = 15.0;
        }
    }

    // More breathing room: bigger button padding, taller interactive elements,
    // and slightly more vertical space between items.
    style.spacing.button_padding   = egui::vec2(10.0, 5.0);  // default: [4, 1]
    style.spacing.item_spacing     = egui::vec2(8.0,  6.0);  // default: [8, 3]
    style.spacing.interact_size.y  = 24.0;                   // default: 18  — taller inputs/combos

    ctx.set_style(style);
}

pub fn run_gui() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("sds-converter")
            .with_inner_size([820.0, 640.0]),
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

/// Document language for SDS extraction and output generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    /// Japanese (日本語) — default
    #[default]
    Japanese,
    /// English
    English,
    /// Simplified Chinese (简体中文)
    ChineseSimplified,
    /// Traditional Chinese (繁體中文)
    ChineseTraditional,
}

impl Language {
    /// BCP-47 language tag.
    pub fn bcp47(&self) -> &'static str {
        match self {
            Self::Japanese => "ja",
            Self::English => "en",
            Self::ChineseSimplified => "zh-CN",
            Self::ChineseTraditional => "zh-TW",
        }
    }

    /// Human-readable name of the language in English.
    pub fn name_en(&self) -> &'static str {
        match self {
            Self::Japanese => "Japanese",
            Self::English => "English",
            Self::ChineseSimplified => "Simplified Chinese",
            Self::ChineseTraditional => "Traditional Chinese",
        }
    }
}

/// Heuristically detect the language of an SDS document from its extracted text.
///
/// Detection order:
/// 1. Hiragana or katakana present → [`Language::Japanese`]
/// 2. Fewer than 20 CJK characters → [`Language::English`]
/// 3. Traditional-Chinese-only characters outnumber simplified-only → [`Language::ChineseTraditional`]
/// 4. Otherwise → [`Language::ChineseSimplified`]
///
/// Works on as little as ~200 characters of text. No LLM or network call required.
pub fn detect_language(text: &str) -> Language {
    // Hiragana (あ…ん) and katakana (ア…ン) are unique to Japanese.
    let japanese_kana = text
        .chars()
        .filter(|&c| matches!(c, '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}'))
        .count();
    if japanese_kana > 5 {
        return Language::Japanese;
    }

    // Count CJK unified ideographs.
    let cjk_total = text
        .chars()
        .filter(|&c| matches!(c, '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}'))
        .count();
    if cjk_total < 20 {
        return Language::English;
    }

    // Distinguish Simplified vs Traditional Chinese by counting characters that diverge
    // between the two writing systems.  Each entry is a Simplified char whose Traditional
    // counterpart is the corresponding entry in TRADITIONAL_MARKERS (same index).
    const SIMPLIFIED_MARKERS: &[char] = &[
        '国', '语', '时', '书', '来', '这', '过', '东', '样', '从',
        '实', '动', '产', '问', '给', '长', '发', '规', '药', '标',
        '剂', '险', '质', '现', '处', '须', '经', '联', '则', '级',
        '为', '与', '对', '气', '无', '变', '数', '间', '应', '关',
    ];
    const TRADITIONAL_MARKERS: &[char] = &[
        '國', '語', '時', '書', '來', '這', '過', '東', '樣', '從',
        '實', '動', '產', '問', '給', '長', '發', '規', '藥', '標',
        '劑', '險', '質', '現', '處', '須', '經', '聯', '則', '級',
        '為', '與', '對', '氣', '無', '變', '數', '間', '應', '關',
    ];

    let simplified_score = text.chars().filter(|c| SIMPLIFIED_MARKERS.contains(c)).count();
    let traditional_score = text.chars().filter(|c| TRADITIONAL_MARKERS.contains(c)).count();

    if traditional_score > simplified_score {
        Language::ChineseTraditional
    } else {
        Language::ChineseSimplified
    }
}

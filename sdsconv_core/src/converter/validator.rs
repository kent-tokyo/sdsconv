use crate::country::SourceCountry;
use crate::ghs_codes;
use crate::schema::{HazardIdentificationClassification, SdsRoot};

// ---------------------------------------------------------------------------
// Typed finding
// ---------------------------------------------------------------------------

/// A structured validation finding.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Finding {
    pub level:   String,   // "CRIT" | "HIGH" | "MED" | "LOW" | "WARN"
    pub rule:    String,   // e.g. "S2-GHS-INCOMPLETE", "STRUCTURAL", "SourceVerify"
    pub message: String,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.rule.as_str() {
            "STRUCTURAL" => write!(f, "{}", self.message),
            rule if self.level == "WARN" && !rule.starts_with('S') => {
                write!(f, "[{}] {}", rule, self.message)
            }
            _ => write!(f, "[{}][{}] {}", self.level, self.rule, self.message),
        }
    }
}

/// Parse a single warning string into a typed [`Finding`].
fn parse_finding(w: &str) -> Finding {
    // [LEVEL][RULE] message
    if let Some(rest) = w.strip_prefix('[') {
        if let Some(level_end) = rest.find(']') {
            let level = &rest[..level_end];
            let after = &rest[level_end + 1..];
            if let Some(rule_part) = after.strip_prefix('[') {
                if let Some(rule_end) = rule_part.find(']') {
                    return Finding {
                        level:   level.to_string(),
                        rule:    rule_part[..rule_end].to_string(),
                        message: rule_part[rule_end + 2..].trim().to_string(),
                    };
                }
            }
            // [SourceVerify] / [China] / [Korea] / [Taiwan]
            return Finding {
                level:   "WARN".to_string(),
                rule:    level.to_string(),
                message: after.trim_start_matches(']').trim().to_string(),
            };
        }
    }
    // plain structural warning
    Finding {
        level:   "WARN".to_string(),
        rule:    "STRUCTURAL".to_string(),
        message: w.to_string(),
    }
}

/// Typed variant of [`validate`]. Returns structured [`Finding`]s instead of raw strings.
pub fn validate_typed(sds: &SdsRoot) -> Vec<Finding> {
    validate(sds).iter().map(|s| parse_finding(s)).collect()
}

// ── Phase 1 helpers ──────────────────────────────────────────────────────────

/// Returns true if `s` looks like a date (contains a 4-digit year or a Japanese era year).
fn looks_like_date(s: &str) -> bool {
    let s = s.trim();
    // Western year: 4-digit (YYYY) or 8-digit (YYYYMMDD) sequence starting with 1 or 2.
    if s.split(|c: char| !c.is_ascii_digit())
        .any(|tok| {
            (tok.len() == 4 || tok.len() == 8)
                && tok.starts_with(|c: char| c == '1' || c == '2')
        })
    {
        return true;
    }
    // Japanese era date: contains "年" (year kanji) and at least one digit.
    if s.contains('年') && s.chars().any(|c| c.is_ascii_digit()) {
        return true;
    }
    false
}

/// Placeholder / generic values that should not appear as product names.
const PLACEHOLDER_NAMES: &[&str] = &[
    "n/a", "na", "unknown", "不明", "未定", "化学物質", "chemical substance",
    "substance", "物質名", "product", "製品名",
];

fn is_placeholder(s: &str) -> bool {
    let lower = s.trim().to_lowercase();
    PLACEHOLDER_NAMES.contains(&lower.as_str()) || lower.is_empty()
}

/// "非危険物" を意味する分類値。これらだけが存在する場合は実質的な危険分類なし。
const NOT_CLASSIFIED_PATTERNS: &[&str] = &[
    "not classified", "not applicable", "n/a", "na",
    "分類できない", "分類対象外", "区分外", "該当なし", "データなし",
    "不适用", "不分类", "无资料",
    // 「分類不明 / 分类不明」= data unavailable, treat same as not-classified
    "分類不明", "分类不明",
];

fn is_not_classified(s: &str) -> bool {
    let lower = s.trim().to_lowercase();
    NOT_CLASSIFIED_PATTERNS.iter().any(|&p| lower.contains(p)) || lower.is_empty()
}

/// Returns true if Classification contains at least one real hazard category
/// (i.e., at least one field that is not empty / "not classified" / "not applicable").
fn classification_has_real_hazard(c: &HazardIdentificationClassification) -> bool {
    let Ok(v) = serde_json::to_value(c) else { return false };
    let mut strings: Vec<String> = Vec::new();
    collect_strings(&v, &mut strings);
    strings.iter().any(|s| !is_not_classified(s))
}

/// Collect every string leaf in a serde_json::Value tree into `out`.
fn collect_strings(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s) => out.push(s.clone()),
        serde_json::Value::Object(map) => map.values().for_each(|v| collect_strings(v, out)),
        serde_json::Value::Array(arr) => arr.iter().for_each(|v| collect_strings(v, out)),
        _ => {}
    }
}

/// Performs post-deserialization structural checks on all 16 SDS sections.
/// Does not hard-fail so partial results are still usable.
pub fn validate(sds: &SdsRoot) -> Vec<String> {
    let mut w: Vec<String> = Vec::new();

    macro_rules! missing {
        ($section:literal) => {
            w.push(format!(
                "{}: section not extracted — check source document.",
                $section
            ))
        };
    }

    // ── Section 1: Identification ─────────────────────────────────────────────
    match &sds.identification {
        None => missing!("Section 1 (Identification)"),
        Some(id) => {
            let has_name = id
                .trade_product_identity
                .as_ref()
                .map(|t| t.trade_name_jp.is_some() || t.trade_name_en.is_some())
                .unwrap_or(false);
            if !has_name {
                w.push("Section 1 (Identification): no product name (TradeNameJP/TradeNameEN).".into());
            }
            if id.supplier_information.is_none() {
                w.push("Section 1 (Identification): SupplierInformation is missing.".into());
            }
        }
    }

    // ── Section 2: HazardIdentification ──────────────────────────────────────
    match &sds.hazard_identification {
        None => missing!("Section 2 (HazardIdentification)"),
        Some(hz) => {
            if hz.classification.is_none() && hz.hazard_labelling.is_none() {
                w.push("Section 2 (HazardIdentification): neither Classification nor HazardLabelling extracted.".into());
            }
            if let Some(hl) = &hz.hazard_labelling {
                // GHS H-code validation
                if let Some(stmts) = &hl.hazard_statement {
                    for s in stmts {
                        if let Some(code) = &s.hazard_statement_code {
                            let upper = code.to_uppercase();
                            if !ghs_codes::is_valid_h_code(&upper) {
                                w.push(format!(
                                    "Section 2 (HazardStatement): unknown H-code '{code}'"
                                ));
                            }
                        }
                    }
                }
                // GHS P-code validation
                if let Some(ps) = &hl.precautionary_statements {
                    let all_codes: Vec<Option<&String>> = ps
                        .prevention
                        .iter()
                        .flatten()
                        .map(|s| s.precautionary_statement_code.as_ref())
                        .chain(
                            ps.response
                                .iter()
                                .flatten()
                                .map(|s| s.precautionary_statement_code.as_ref()),
                        )
                        .chain(
                            ps.storage
                                .iter()
                                .flatten()
                                .map(|s| s.precautionary_statement_code.as_ref()),
                        )
                        .chain(
                            ps.disposal
                                .iter()
                                .flatten()
                                .map(|s| s.precautionary_statement_code.as_ref()),
                        )
                        .collect();
                    for code in all_codes.into_iter().flatten() {
                        let upper = code.to_uppercase();
                        if !ghs_codes::is_valid_p_code(&upper) {
                            w.push(format!(
                                "Section 2 (PrecautionaryStatement): unknown P-code '{code}'"
                            ));
                        }
                    }
                }
            }
        }
    }

    // ── Section 3: Composition ────────────────────────────────────────────────
    match &sds.composition {
        None => missing!("Section 3 (Composition)"),
        Some(comp) => {
            let items = comp.composition_and_concentration.as_deref().unwrap_or(&[]);
            if items.is_empty() {
                w.push("Section 3 (Composition): CompositionAndConcentration is empty.".into());
            }
            // CAS number format and check-digit validation
            for (i, item) in items.iter().enumerate() {
                if let Some(ids) = &item.substance_identifiers {
                    if let Some(identity) = &ids.substance_identity {
                        if let Some(cas_node) = &identity.ca_sno {
                            for cas in cas_node.full_text.iter().flatten() {
                                match validate_cas(cas) {
                                    CasValidation::Ok => {}
                                    CasValidation::InvalidFormat => {
                                        w.push(format!(
                                            "Section 3 (Composition[{i}]): invalid CAS format '{cas}' \
                                             (expected \\d{{2,7}}-\\d{{2}}-\\d)"
                                        ));
                                    }
                                    CasValidation::InvalidCheckDigit { expected } => {
                                        w.push(format!(
                                            "Section 3 (Composition[{i}]): CAS check digit mismatch \
                                             '{cas}' (expected check digit {expected}, source has {})",
                                            cas.chars().last().unwrap_or('?')
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Section 4: FirstAidMeasures ───────────────────────────────────────────
    if sds.first_aid_measures.is_none() {
        missing!("Section 4 (FirstAidMeasures)");
    }

    // ── Section 5: FireFightingMeasures ───────────────────────────────────────
    if sds.fire_fighting_measures.is_none() {
        missing!("Section 5 (FireFightingMeasures)");
    }

    // ── Section 6: AccidentalReleaseMeasures ─────────────────────────────────
    if sds.accidental_release_measures.is_none() {
        missing!("Section 6 (AccidentalReleaseMeasures)");
    }

    // ── Section 7: HandlingAndStorage ────────────────────────────────────────
    if sds.handling_and_storage.is_none() {
        missing!("Section 7 (HandlingAndStorage)");
    }

    // ── Section 8: ExposureControlPersonalProtection ─────────────────────────
    if sds.exposure_control_personal_protection.is_none() {
        missing!("Section 8 (ExposureControlPersonalProtection)");
    }

    // ── Section 9: PhysicalChemicalProperties ────────────────────────────────
    if sds.physical_chemical_properties.is_none() {
        missing!("Section 9 (PhysicalChemicalProperties)");
    }

    // ── Section 10: StabilityReactivity ──────────────────────────────────────
    match &sds.stability_reactivity {
        None => missing!("Section 10 (StabilityReactivity)"),
        Some(sr) => {
            if sr.stability_description.is_none() && sr.reactivity_description.is_none() {
                w.push("Section 10 (StabilityReactivity): neither StabilityDescription nor ReactivityDescription extracted.".into());
            }
        }
    }

    // ── Section 11: ToxicologicalInformation ─────────────────────────────────
    match &sds.toxicological_information {
        None => missing!("Section 11 (ToxicologicalInformation)"),
        Some(list) if list.is_empty() => {
            w.push("Section 11 (ToxicologicalInformation): array is present but empty.".into());
        }
        _ => {}
    }

    // ── Section 12: EcologicalInformation ────────────────────────────────────
    match &sds.ecological_information {
        None => missing!("Section 12 (EcologicalInformation)"),
        Some(list) if list.is_empty() => {
            w.push("Section 12 (EcologicalInformation): array is present but empty.".into());
        }
        _ => {}
    }

    // ── Section 13: DisposalConsiderations ───────────────────────────────────
    if sds.disposal_considerations.is_none() {
        missing!("Section 13 (DisposalConsiderations)");
    }

    // ── Section 14: TransportInformation ─────────────────────────────────────
    if sds.transport_information.is_none() {
        missing!("Section 14 (TransportInformation)");
    }

    // ── Section 15: RegulatoryInformation ────────────────────────────────────
    if sds.regulatory_information.is_none() {
        missing!("Section 15 (RegulatoryInformation)");
    }

    // ── Section 16: OtherInformation ─────────────────────────────────────────
    if sds.other_information.is_none() {
        missing!("Section 16 (OtherInformation)");
    }

    // ── Phase 1: cross-section consistency & value sanity ────────────────────

    // Date format: IssueDate / RevisionDate
    if let Some(ds) = &sds.datasheet {
        if let Some(d) = &ds.issue_date {
            if !looks_like_date(d) {
                w.push(format!("Datasheet.IssueDate '{d}' does not look like a valid date."));
            }
        }
        for d in ds.revision_date.iter().flatten() {
            if !looks_like_date(d) {
                w.push(format!("Datasheet.RevisionDate '{d}' does not look like a valid date."));
            }
        }
    }

    // ProductName quality
    if let Some(id) = &sds.identification {
        if let Some(tpi) = &id.trade_product_identity {
            for (field, val) in [
                ("TradeNameJP", tpi.trade_name_jp.as_deref()),
                ("TradeNameEN", tpi.trade_name_en.as_deref()),
            ] {
                if let Some(name) = val {
                    if is_placeholder(name) {
                        w.push(format!(
                            "Section 1 (Identification.{field}): value '{name}' looks like a placeholder."
                        ));
                    }
                }
            }
        }
    }

    // Concentration range: numeric % values should be 0-100
    if let Some(comp) = &sds.composition {
        for (i, item) in comp
            .composition_and_concentration
            .iter()
            .flatten()
            .enumerate()
        {
            if let Some(conc) = &item.concentration {
                if let Some(nr) = &conc.numeric_range_with_unit_and_qualifier {
                    let is_percent = nr
                        .unit
                        .as_deref()
                        .map(|u| u.contains('%'))
                        .unwrap_or(false);
                    if is_percent {
                        let values: Vec<f64> = [
                            nr.exact_value.as_ref().and_then(|v| v.value),
                            nr.lower_value.as_ref().and_then(|v| v.value),
                            nr.upper_value.as_ref().and_then(|v| v.value),
                        ]
                        .into_iter()
                        .flatten()
                        .collect();
                        for v in values {
                            if !(0.0..=100.0).contains(&v) {
                                w.push(format!(
                                    "Section 3 (Composition[{i}]): concentration {v}% is outside the \
                                     valid range 0-100."
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // Cross-consistency: real hazard Classification present but HazardStatement absent.
    // Skip non-hazardous substances (all categories "not classified" / "not applicable").
    if let Some(hz) = &sds.hazard_identification {
        let has_real_classification = hz
            .classification
            .as_ref()
            .map(classification_has_real_hazard)
            .unwrap_or(false);
        let has_hazard_statement = hz
            .hazard_labelling
            .as_ref()
            .and_then(|hl| hl.hazard_statement.as_ref())
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        if has_real_classification && !has_hazard_statement {
            w.push(
                "Section 2 (HazardIdentification): Classification is present but \
                 HazardStatement is absent — verify labelling completeness."
                    .into(),
            );
        }
    }

    // ── MHLW §3 compliance rules ─────────────────────────────────────────────

    // [HIGH] S2-GHS-INCOMPLETE: H-codes present but GHS labelling elements missing.
    if let Some(hz) = &sds.hazard_identification {
        if let Some(hl) = &hz.hazard_labelling {
            let has_h_codes = hl
                .hazard_statement
                .as_deref()
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            if has_h_codes {
                if hl.hazard_pictogram.as_deref().map(|p| p.is_empty()).unwrap_or(true) {
                    w.push(
                        "[HIGH][S2-GHS-INCOMPLETE] Section 2: HazardStatement present but \
                         HazardPictogram is empty — GHS labelling incomplete."
                            .into(),
                    );
                }
                if hl.signal_word.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true) {
                    w.push(
                        "[HIGH][S2-GHS-INCOMPLETE] Section 2: HazardStatement present but \
                         SignalWord is missing — GHS labelling incomplete."
                            .into(),
                    );
                }
                if hz.classification.is_none() {
                    w.push(
                        "[HIGH][S2-GHS-INCOMPLETE] Section 2: HazardStatement present but \
                         Classification is missing — MHLW format requires both."
                            .into(),
                    );
                }
            }
        }
    }

    // [HIGH] S3-CAS-WITHOUT-NAME / S3-NAME-WITHOUT-CAS.
    if let Some(comp) = &sds.composition {
        for (i, item) in comp
            .composition_and_concentration
            .iter()
            .flatten()
            .enumerate()
        {
            if let Some(ids) = &item.substance_identifiers {
                let has_cas = ids
                    .substance_identity
                    .as_ref()
                    .and_then(|si| si.ca_sno.as_ref())
                    .and_then(|c| c.full_text.as_ref())
                    .map(|ft| ft.iter().any(|s| !s.trim().is_empty()))
                    .unwrap_or(false);
                let has_name = ids
                    .substance_names
                    .as_ref()
                    .map(|n| {
                        n.iupac_name.as_deref().is_some_and(|s| !s.trim().is_empty())
                            || n.generic_name.as_deref().is_some_and(|s| !s.trim().is_empty())
                    })
                    .unwrap_or(false)
                    || ids
                        .common_name
                        .as_ref()
                        .and_then(|cn| cn.other_name.as_ref())
                        .map(|names| names.iter().any(|s| !s.trim().is_empty()))
                        .unwrap_or(false);
                if has_cas && !has_name {
                    w.push(format!(
                        "[HIGH][S3-CAS-WITHOUT-NAME] Section 3 (Composition[{i}]): \
                         CAS number present but substance name is missing."
                    ));
                } else if has_name && !has_cas {
                    w.push(format!(
                        "[HIGH][S3-NAME-WITHOUT-CAS] Section 3 (Composition[{i}]): \
                         substance name present but CAS number is missing."
                    ));
                }
            }
        }
    }

    // [HIGH] S14-UN-INCOMPLETE: UN number present but required transport fields missing.
    if let Some(ti) = &sds.transport_information {
        for regs in ti.international_regulations.iter().flatten() {
            for reg_name in regs.regulation_name.iter().flatten() {
                if let Some(un) = &reg_name.un_no {
                    if !un.trim().is_empty() {
                        if reg_name.proper_shipping_name.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true) {
                            w.push(format!(
                                "[HIGH][S14-UN-INCOMPLETE] Section 14: UN No. '{un}' present but \
                                 ProperShippingName is missing."
                            ));
                        }
                        if reg_name.packing_group.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true) {
                            w.push(format!(
                                "[HIGH][S14-UN-INCOMPLETE] Section 14: UN No. '{un}' present but \
                                 PackingGroup is missing."
                            ));
                        }
                    }
                }
            }
        }
    }

    // [MED] S2-DUPLICATE-HCODE / S2-DUPLICATE-PCODE.
    if let Some(hz) = &sds.hazard_identification {
        if let Some(hl) = &hz.hazard_labelling {
            // H-codes
            let h_codes: Vec<String> = hl
                .hazard_statement
                .iter()
                .flatten()
                .filter_map(|s| s.hazard_statement_code.clone())
                .map(|c| c.to_uppercase())
                .collect();
            let mut seen_h = std::collections::HashSet::new();
            for code in &h_codes {
                if !seen_h.insert(code.as_str()) {
                    w.push(format!(
                        "[MED][S2-DUPLICATE-HCODE] Section 2: H-code '{code}' appears more than once."
                    ));
                }
            }
            // P-codes — each sub-list has a distinct concrete type; collect codes separately.
            if let Some(ps) = &hl.precautionary_statements {
                let mut p_codes: Vec<String> = Vec::new();
                for s in ps.prevention.iter().flatten() {
                    if let Some(c) = &s.precautionary_statement_code { p_codes.push(c.to_uppercase()); }
                }
                for s in ps.response.iter().flatten() {
                    if let Some(c) = &s.precautionary_statement_code { p_codes.push(c.to_uppercase()); }
                }
                for s in ps.storage.iter().flatten() {
                    if let Some(c) = &s.precautionary_statement_code { p_codes.push(c.to_uppercase()); }
                }
                for s in ps.disposal.iter().flatten() {
                    if let Some(c) = &s.precautionary_statement_code { p_codes.push(c.to_uppercase()); }
                }
                let mut seen_p = std::collections::HashSet::new();
                for code in &p_codes {
                    if !seen_p.insert(code.as_str()) {
                        w.push(format!(
                            "[MED][S2-DUPLICATE-PCODE] Section 2: P-code '{code}' appears more than once."
                        ));
                    }
                }
            }
        }
    }

    // [MED] S9-VALUE-WITHOUT-UNIT: numeric physical property value without a unit.
    if let Some(pcp) = &sds.physical_chemical_properties {
        let Ok(v) = serde_json::to_value(pcp) else { return w };
        check_numeric_without_unit(&v, &mut w);
    }

    w
}

/// Walk a JSON value tree and warn for every `NumericRangeWithUnitAndQualifier`-shaped
/// object that has a numeric value but no `Unit` field.
fn check_numeric_without_unit(v: &serde_json::Value, w: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(map) => {
            let has_value = map.contains_key("ExactValue")
                || map.contains_key("UpperValue")
                || map.contains_key("LowerValue");
            let has_unit = map
                .get("Unit")
                .and_then(|u| u.as_str())
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            let has_additional_info = map.contains_key("AdditionalInfo");
            if has_value && !has_unit && !has_additional_info {
                w.push(
                    "[MED][S9-VALUE-WITHOUT-UNIT] Section 9: numeric physical property \
                     value present without a Unit — add unit (e.g. \"°C\", \"kPa\", \"g/cm³\")."
                        .into(),
                );
            }
            for child in map.values() {
                check_numeric_without_unit(child, w);
            }
        }
        serde_json::Value::Array(arr) => {
            for child in arr {
                check_numeric_without_unit(child, w);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Typed findings for the correction pass
// ---------------------------------------------------------------------------

/// Which sub-list a P-code belongs to within PrecautionaryStatements.
#[derive(Debug, Clone, PartialEq)]
pub enum PCodeCategory {
    Prevention,
    Response,
    Storage,
    Disposal,
}

impl std::fmt::Display for PCodeCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Prevention => write!(f, "Prevention"),
            Self::Response   => write!(f, "Response"),
            Self::Storage    => write!(f, "Storage"),
            Self::Disposal   => write!(f, "Disposal"),
        }
    }
}

/// An actionable validation finding with enough context to apply a correction.
///
/// Unlike the string-based [`validate()`] warnings, these carry the structured
/// field path and the pre-computed correct value so [`crate::converter::corrector`]
/// can apply fixes without re-parsing warning messages.
#[derive(Debug, Clone)]
pub enum ValidationFinding {
    /// An H-code in `HazardLabelling.HazardStatement` is not in the GHS H-code list.
    UnknownHCode {
        /// The invalid code as extracted (e.g. `"H999"`).
        code: String,
        /// Full text of the hazard statement (for LLM context).
        full_text: Option<String>,
        /// Zero-based index into `HazardStatement[]`.
        statement_index: usize,
    },
    /// A P-code in `PrecautionaryStatements` is not in the GHS P-code list.
    UnknownPCode {
        /// The invalid code as extracted (e.g. `"P286"`).
        code: String,
        /// Full text of the precautionary statement (for LLM context).
        full_text: Option<String>,
        /// Which sub-list the entry belongs to.
        category: PCodeCategory,
        /// Zero-based index within that sub-list.
        statement_index: usize,
    },
    /// A CAS number whose check digit does not match; `expected_digit` is already computed.
    CasCheckDigit {
        /// The CAS string with the wrong check digit (e.g. `"238016-30-4"`).
        cas: String,
        /// Zero-based index into `CompositionAndConcentration[]`.
        composition_index: usize,
        /// The digit the weighted-sum algorithm requires.
        expected_digit: u32,
    },
}

/// Collect actionable [`ValidationFinding`]s for Sections 2 (GHS codes) and 3 (CAS).
///
/// These are the findings that [`crate::converter::corrector::apply_correction_pass`]
/// can act on.  Structural presence-check warnings (missing sections, missing supplier
/// info, etc.) are **not** included here because they cannot be corrected without
/// regenerating the entire section.
///
/// This function is independent of [`validate()`] and can be called before or after
/// it without affecting the existing warning pipeline.
pub fn collect_findings(sds: &SdsRoot) -> Vec<ValidationFinding> {
    let mut findings: Vec<ValidationFinding> = Vec::new();

    // ── Section 2: H-codes ────────────────────────────────────────────────────
    if let Some(hz) = &sds.hazard_identification {
        if let Some(hl) = &hz.hazard_labelling {
            // H-codes
            if let Some(stmts) = &hl.hazard_statement {
                for (i, s) in stmts.iter().enumerate() {
                    if let Some(code) = &s.hazard_statement_code {
                        if !ghs_codes::is_valid_h_code(&code.to_uppercase()) {
                            findings.push(ValidationFinding::UnknownHCode {
                                code: code.clone(),
                                full_text: s.full_text.clone(),
                                statement_index: i,
                            });
                        }
                    }
                }
            }
            // P-codes — each sub-list has a distinct concrete type, so we use a
            // macro to avoid repeating identical logic four times.
            if let Some(ps) = &hl.precautionary_statements {
                macro_rules! collect_p {
                    ($cat:expr, $list:expr) => {
                        if let Some(list) = $list {
                            for (i, s) in list.iter().enumerate() {
                                if let Some(code) = &s.precautionary_statement_code {
                                    if !ghs_codes::is_valid_p_code(&code.to_uppercase()) {
                                        findings.push(ValidationFinding::UnknownPCode {
                                            code: code.clone(),
                                            full_text: s.full_text.clone(),
                                            category: $cat,
                                            statement_index: i,
                                        });
                                    }
                                }
                            }
                        }
                    };
                }
                collect_p!(PCodeCategory::Prevention, ps.prevention.as_deref());
                collect_p!(PCodeCategory::Response,   ps.response.as_deref());
                collect_p!(PCodeCategory::Storage,    ps.storage.as_deref());
                collect_p!(PCodeCategory::Disposal,   ps.disposal.as_deref());
            }
        }
    }

    // ── Section 3: CAS check-digit mismatches ─────────────────────────────────
    if let Some(comp) = &sds.composition {
        let items = comp.composition_and_concentration.as_deref().unwrap_or(&[]);
        for (i, item) in items.iter().enumerate() {
            if let Some(ids) = &item.substance_identifiers {
                if let Some(identity) = &ids.substance_identity {
                    if let Some(cas_node) = &identity.ca_sno {
                        for cas in cas_node.full_text.iter().flatten() {
                            if let CasValidation::InvalidCheckDigit { expected } = validate_cas(cas) {
                                findings.push(ValidationFinding::CasCheckDigit {
                                    cas: cas.clone(),
                                    composition_index: i,
                                    expected_digit: expected,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    findings
}

/// Detailed result of CAS Registry Number validation.
#[derive(Debug, PartialEq)]
pub(crate) enum CasValidation {
    /// The CAS number passes both format and check-digit validation.
    Ok,
    /// The string does not match the expected format (`\d{2,7}-\d{2}-\d`).
    InvalidFormat,
    /// The format is correct but the check digit is wrong.
    InvalidCheckDigit {
        /// The check digit value that the weighted-sum algorithm requires.
        expected: u32,
    },
}

/// Validate a CAS Registry Number, returning a detailed [`CasValidation`] result.
///
/// Format: `^\d{2,7}-\d{2}-\d$`
/// Check digit: weighted sum of all non-check digits (right-to-left, weight starts at 1)
/// modulo 10 must equal the supplied check digit.
pub(crate) fn validate_cas(cas: &str) -> CasValidation {
    let parts: Vec<&str> = cas.split('-').collect();
    if parts.len() != 3 {
        return CasValidation::InvalidFormat;
    }
    let (a, b, c) = (parts[0], parts[1], parts[2]);
    // Format check
    if a.len() < 2
        || a.len() > 7
        || b.len() != 2
        || c.len() != 1
        || !a.chars().all(|ch| ch.is_ascii_digit())
        || !b.chars().all(|ch| ch.is_ascii_digit())
        || !c.chars().all(|ch| ch.is_ascii_digit())
    {
        return CasValidation::InvalidFormat;
    }
    // Both unwraps are guaranteed by the format check above.
    let check_digit: u32 = c
        .chars()
        .next()
        .expect("c has length 1 after format check")
        .to_digit(10)
        .expect("c is an ASCII digit after format check");
    let digits: Vec<u32> = a
        .chars()
        .chain(b.chars())
        .filter_map(|ch| ch.to_digit(10))
        .collect();
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| d * (i as u32 + 1))
        .sum();
    let expected = sum % 10;
    if expected == check_digit {
        CasValidation::Ok
    } else {
        CasValidation::InvalidCheckDigit { expected }
    }
}

/// Convenience wrapper — returns `true` only when both format and check digit are valid.
///
/// Kept for use by [`crate::enrichment`] which only needs a boolean result.
pub(crate) fn validate_cas_format(cas: &str) -> bool {
    validate_cas(cas) == CasValidation::Ok
}

// ---------------------------------------------------------------------------
// Phase 2: source-text verification (zero LLM calls)
// ---------------------------------------------------------------------------

/// Check whether key extracted values can be found verbatim in the source text.
///
/// This is a pure, deterministic pass — no API calls.  Values that are too
/// short (< 3 chars), match a placeholder pattern, or are otherwise
/// un-verifiable are silently skipped.
///
/// Returns human-readable warnings for each value that could not be located in
/// `source_text`.  These warnings should be appended to the conversion report
/// alongside the `validate()` warnings.
/// Country-specific validation checks run after the universal `validate()` pass.
///
/// Returns a list of warning strings prefixed with `[China]`, `[Korea]`, etc.
/// so they are distinguishable in the conversion report.
pub fn validate_country(sds: &SdsRoot, country: SourceCountry) -> Vec<String> {
    let mut w: Vec<String> = Vec::new();
    match country {
        SourceCountry::China => {
            if !has_emergency_contact(sds) {
                w.push(
                    "WARN: [China] Section 1: 24-hour emergency contact (紧急电话) is \
                     mandatory under GB/T 16483 but was not found."
                        .into(),
                );
            }
            if !regulatory_mentions_keyword(sds, &["危险化学品", "GB 13690", "GB13690", "GB 30000"]) {
                w.push(
                    "WARN: [China] Section 15: GB/T 16483 requires reference to \
                     危险化学品目录 / GB 13690 in RegulatoryInformation, but none found."
                        .into(),
                );
            }
        }
        SourceCountry::Korea => {
            if !has_emergency_contact(sds) {
                w.push(
                    "WARN: [Korea] Section 1: emergency contact (e.g. 1588-9119) is \
                     required by K-GHS but was not found."
                        .into(),
                );
            }
        }
        SourceCountry::Taiwan => {
            if !has_emergency_contact(sds) {
                w.push(
                    "WARN: [Taiwan] Section 1: emergency contact information is \
                     required by CNS 15030 but was not found."
                        .into(),
                );
            }
        }
        SourceCountry::Japan => {}
    }
    w
}

/// Returns true if any EmergencyContact entry with a non-empty Phone field is present.
fn has_emergency_contact(sds: &SdsRoot) -> bool {
    let id = match sds.identification.as_ref() { Some(v) => v, None => return false };
    // Check DomesticManufacturerInformation.EmergencyContact
    if let Some(dmi) = &id.domestic_manufacturer_information {
        if let Some(contacts) = &dmi.emergency_contact {
            if contacts.iter().any(|c| c.phone.as_ref().map_or(false, |p| !p.is_empty())) {
                return true;
            }
        }
    }
    // Check SupplierInformation.EmergencyContact
    if let Some(si) = &id.supplier_information {
        if let Some(contacts) = &si.emergency_contact {
            if contacts.iter().any(|c| c.phone.as_ref().map_or(false, |p| !p.is_empty())) {
                return true;
            }
        }
    }
    false
}

/// Returns true if RegulatoryInformation contains any of the given keywords.
fn regulatory_mentions_keyword(sds: &SdsRoot, keywords: &[&str]) -> bool {
    let ri = match sds.regulatory_information.as_ref() { Some(v) => v, None => return false };
    // Serialize to JSON and do a string search — simpler than traversing the deeply nested schema.
    let Ok(v) = serde_json::to_string(ri) else { return false };
    keywords.iter().any(|kw| v.contains(kw))
}

pub fn verify_against_source(sds: &SdsRoot, source_text: &str, skip_cas: &[String]) -> Vec<String> {
    let mut w: Vec<String> = Vec::new();

    // ── ProductName ──────────────────────────────────────────────────────────
    let source_has_kana = has_kana(source_text);
    if let Some(id) = &sds.identification {
        if let Some(tpi) = &id.trade_product_identity {
            for (field, val) in [
                ("TradeNameJP", tpi.trade_name_jp.as_deref()),
                ("TradeNameEN", tpi.trade_name_en.as_deref()),
            ] {
                if let Some(name) = val {
                    if !is_placeholder(name) && !source_contains_name(source_text, name) {
                        // Extra hint when TradeNameJP is absent from a kana-free source:
                        // the LLM likely invented a Japanese name (kana or kanji) that
                        // does not appear anywhere in the source document.
                        let suffix = if field == "TradeNameJP" && !source_has_kana {
                            " (source has no Japanese — likely a language hallucination)"
                        } else {
                            " — possible hallucination"
                        };
                        w.push(format!(
                            "[SourceVerify] Section 1 ({field}): '{}' not found verbatim in \
                             source text{suffix}.",
                            truncate(name, 60)
                        ));
                    }
                }
            }
        }
    }

    // ── CAS numbers ─────────────────────────────────────────────────────────
    if let Some(comp) = &sds.composition {
        for (i, item) in comp
            .composition_and_concentration
            .iter()
            .flatten()
            .enumerate()
        {
            if let Some(ids) = &item.substance_identifiers {
                if let Some(identity) = &ids.substance_identity {
                    if let Some(cas_node) = &identity.ca_sno {
                        for cas in cas_node.full_text.iter().flatten() {
                            // Only check format-valid CAS numbers; malformed ones
                            // (e.g. "无资料") are already reported by validate().
                            // Skip CAS values that were deterministically corrected by the
                            // corrector — the source PDF has the wrong digit, so a match
                            // against the corrected value would always fail.
                            if validate_cas(cas) == CasValidation::Ok
                                && !skip_cas.contains(cas)
                                && !source_contains_cas(source_text, cas)
                            {
                                w.push(format!(
                                    "[SourceVerify] Section 3 (Composition[{i}] CAS): \
                                     '{cas}' not found in source text — possible hallucination."
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Substance names ──────────────────────────────────────────────────────
    if let Some(comp) = &sds.composition {
        for (i, item) in comp
            .composition_and_concentration
            .iter()
            .flatten()
            .enumerate()
        {
            if let Some(ids) = &item.substance_identifiers {
                if let Some(names) = &ids.substance_names {
                    for (field, val) in [
                        ("IupacName", names.iupac_name.as_deref()),
                        ("GenericName", names.generic_name.as_deref()),
                    ] {
                        if let Some(name) = val {
                            if name.trim().len() >= 3
                                && !is_placeholder(name)
                                && !source_contains_name(source_text, name)
                            {
                                w.push(format!(
                                    "[SourceVerify] Section 3 (Composition[{i}].{field}): \
                                     '{}' not found verbatim in source text.",
                                    truncate(name, 60)
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Language-mismatch: Japanese kana in non-Japanese source ──────────────
    // If any name field contains hiragana/katakana but the source document has
    // none, the LLM has fabricated a Japanese transliteration that isn't in the
    // source — a hallucination pattern distinct from "not found verbatim".
    if !source_has_kana {
        // TradeNameJP with kana
        if let Some(id) = &sds.identification {
            if let Some(tpi) = &id.trade_product_identity {
                if let Some(name) = &tpi.trade_name_jp {
                    if has_kana(name) {
                        w.push(format!(
                            "[SourceVerify] Section 1 (TradeNameJP): '{}' contains Japanese \
                             kana but none appears in the source document — likely a \
                             transliteration hallucination.",
                            truncate(name, 60)
                        ));
                    }
                }
            }
        }
        // IupacName / GenericName with kana
        if let Some(comp) = &sds.composition {
            for (i, item) in comp
                .composition_and_concentration
                .iter()
                .flatten()
                .enumerate()
            {
                if let Some(ids) = &item.substance_identifiers {
                    if let Some(names) = &ids.substance_names {
                        for (field, val) in [
                            ("IupacName",   names.iupac_name.as_deref()),
                            ("GenericName", names.generic_name.as_deref()),
                        ] {
                            if let Some(name) = val {
                                if has_kana(name) {
                                    w.push(format!(
                                        "[SourceVerify] Section 3 (Composition[{i}].{field}): \
                                         '{}' contains Japanese kana but source has none — \
                                         possible language hallucination.",
                                        truncate(name, 60)
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
        // SupplierInformation.CompanyName with kana
        if let Some(id) = &sds.identification {
            if let Some(si) = &id.supplier_information {
                if let Some(name) = &si.company_name {
                    if has_kana(name) {
                        w.push(format!(
                            "[SourceVerify] Section 1 (SupplierInformation.CompanyName): \
                             '{}' contains Japanese kana but source has none — \
                             possible language hallucination.",
                            truncate(name, 60)
                        ));
                    }
                }
            }
        }
    }

    w
}

// ---------------------------------------------------------------------------
// Source-text search helpers
// ---------------------------------------------------------------------------

/// Product-name-specific source check.
///
/// For short names (≤ 40 chars) the full value must appear. For long names — e.g.
/// UN transport names that span multiple PDF lines with intervening labels — only
/// the first 30 characters need to be found. This avoids false positives where a
/// PDF inserts a section label ("英文名称：") in the middle of a long name.
fn source_contains_name(source: &str, name: &str) -> bool {
    if source_contains(source, name) {
        return true;
    }
    let chars: Vec<char> = name.chars().collect();
    if chars.len() > 40 {
        // For long names (UN names, IUPAC names that span PDF lines), verify only
        // the "base" segment — the text up to the first delimiter that commonly
        // introduces sub-clauses in long names: '(', ',', ';', '（'.
        // Falls back to the first 25 chars if no delimiter appears early enough.
        let delimiters = ['(', ',', ';', '（'];
        let cut = chars[..chars.len().min(35)]
            .iter()
            .position(|c| delimiters.contains(c))
            .unwrap_or(25)
            .max(8); // always verify at least 8 chars
        let prefix: String = chars[..cut].iter().collect();
        if source_contains(source, prefix.trim()) {
            return true;
        }
    }
    // Strip leading positional-number prefix (e.g. "1,1,2,3,4,4-" in IUPAC names like
    // "1,1,2,3,4,4-六氯-1,3-丁二烯") and verify the core name in the source.
    // Sources sometimes print only the short form without position descriptors.
    let name_norm = normalize_for_search(name);
    let src_norm = normalize_for_search(source);
    let core = name_norm.trim_start_matches(|c: char| c.is_ascii_digit() || c == ',' || c == '-');
    if core.len() >= 8 && src_norm.contains(core) {
        return true;
    }
    // Strip optical-rotation / stereodescriptor markers that CID-font PDFs often
    // split across lines, causing the marker to disappear from extracted text.
    // e.g. "イソプロピルβD(-)チオガラクトピラノシド" → "イソプロピルβDチオガラクトピラノシド"
    let stripped_stereo = name_norm
        .replace("(-)", "")
        .replace("(+)", "")
        .replace("(±)", "")
        .replace("(R)", "")
        .replace("(S)", "")
        .replace("(d)", "")
        .replace("(l)", "");
    if stripped_stereo.len() >= 8 && src_norm.contains(stripped_stereo.as_str()) {
        return true;
    }
    false
}

/// Check if `value` appears as a substring of `source` (after whitespace normalization).
///
/// Skips values shorter than 3 characters to avoid trivial false-positives.
/// Falls back to a space-collapsed comparison to handle CID-font PDFs that insert
/// stray spaces inside words (e.g. "エ チル" vs "エチル").
fn source_contains(source: &str, value: &str) -> bool {
    let v = value.trim();
    if v.len() < 3 {
        return true; // too short to verify reliably
    }
    if source.contains(v) {
        return true;
    }
    // Collapse whitespace + normalize fullwidth punctuation, then retry.
    // Handles CID-font artifacts (stray spaces, fullwidth commas/hyphens).
    let v_norm = normalize_for_search(v);
    let src_norm = normalize_for_search(source);
    if v_norm.len() >= 3 {
        src_norm.contains(&v_norm)
    } else {
        false
    }
}

/// Normalize a string for fuzzy source-text matching:
/// - Remove all whitespace
/// - Convert fullwidth ASCII punctuation (，、。．－) to their halfwidth equivalents
/// - Map common traditional Chinese characters to their simplified equivalents so that
///   LLM output using trad. chars still matches simplified-Chinese source PDFs (and vice versa).
fn normalize_for_search(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            // Fullwidth / Japanese punctuation → ASCII
            '，' => ',',
            '、' => ',',  // Japanese ideographic comma (U+3001)
            '．' => '.',
            '。' => '.',
            '－' => '-',
            '（' => '(',
            '）' => ')',
            // Traditional → Simplified (common in chemical/SDS names)
            '異' => '异',  // iso- prefix (异丙基, 异庚烷, …)
            '環' => '环',  // ring/cycle (环氧, 环己烷, …)
            '鹼' => '碱',  // alkali
            '鹽' => '盐',  // salt
            '鋰' => '锂',  // Li
            '鈉' => '钠',  // Na
            '鉀' => '钾',  // K
            '鐵' => '铁',  // Fe
            '銅' => '铜',  // Cu
            '鋁' => '铝',  // Al
            '銀' => '银',  // Ag
            '鋅' => '锌',  // Zn
            '氫' => '氢',  // hydrogen
            '氯' => '氯',  // chlorine (same)
            '無' => '无',  // none/not
            '劑' => '剂',  // agent/reagent suffix
            '酸' => '酸',  // acid (same)
            '烴' => '烃',  // hydrocarbon
            _ => c,
        })
        .collect()
}

/// Returns true if `s` contains any hiragana (U+3040–U+309F) or katakana (U+30A0–U+30FF).
fn has_kana(s: &str) -> bool {
    s.chars().any(|c| matches!(c, '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}'))
}

/// CAS-aware source search: also tries full-width hyphen variants and space-collapsed
/// forms because Japanese PDFs sometimes render hyphens as U+FF0D (－) or insert
/// stray spaces inside CAS numbers.
fn source_contains_cas(source: &str, cas: &str) -> bool {
    if source.contains(cas) {
        return true;
    }
    // Full-width hyphens (U+FF0D).
    let fullwidth = cas.replace('-', "－");
    if source.contains(&fullwidth) {
        return true;
    }
    // Normalized fallback: collapse spaces + fullwidth punctuation.
    let cas_norm = normalize_for_search(cas);
    let src_norm = normalize_for_search(source);
    src_norm.contains(&cas_norm)
}

/// Truncate a string to `max` chars for warning messages.
fn truncate(s: &str, max: usize) -> &str {
    let mut idx = max;
    while !s.is_char_boundary(idx) && idx > 0 {
        idx -= 1;
    }
    if idx < s.len() { &s[..idx] } else { s }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_date_valid() {
        assert!(looks_like_date("2024-03-15"));
        assert!(looks_like_date("2024/03/15"));
        assert!(looks_like_date("令和6年3月15日"));
        assert!(looks_like_date("20240315"));
    }

    #[test]
    fn test_looks_like_date_invalid() {
        assert!(!looks_like_date("不明"));
        assert!(!looks_like_date("N/A"));
        assert!(!looks_like_date("改訂"));
    }

    #[test]
    fn test_source_contains_cas_ascii_hyphen() {
        let source = "成分: エタノール CAS No. 64-17-5 含有量 99%";
        assert!(source_contains_cas(source, "64-17-5"));
    }

    #[test]
    fn test_source_contains_cas_fullwidth_hyphen() {
        let source = "CAS No. 64－17－5";
        assert!(source_contains_cas(source, "64-17-5"));
    }

    #[test]
    fn test_source_contains_cas_not_present() {
        let source = "CAS No. 7732-18-5";
        assert!(!source_contains_cas(source, "64-17-5"));
    }

    #[test]
    fn test_source_contains_short_value_always_passes() {
        let source = "no matches here";
        assert!(source_contains(source, "AB")); // len < 3 → skip
    }

    #[test]
    fn test_truncate_ascii() {
        assert_eq!(truncate("hello world", 5), "hello");
    }

    #[test]
    fn test_truncate_multibyte() {
        let s = "エタノール"; // 5 chars, 15 bytes
        let t = truncate(s, 6); // 6 bytes → "エタ" (each is 3 bytes)
        assert!(s.starts_with(t));
    }
}

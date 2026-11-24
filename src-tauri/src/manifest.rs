//! Reading and writing `manifest.toml` files.
//!
//! The manifest is the source of truth for project state. Edits are applied
//! onto the parsed TOML document so that unknown fields, comments and value
//! decorations are preserved.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use toml_edit::{Array, DocumentMut, Item, Table, Value};

/// Field values exchanged with the frontend.
///
/// All strings: the frontend sends raw input, the backend validates and
/// converts. Invalid values are reported per field instead of being silently
/// discarded.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestFields {
    pub id: String,
    pub title: String,
    pub year: String,
    #[serde(rename = "type")]
    pub project_type: String,
    pub status: String,
    pub documentation_level: String,
    pub keywords: Vec<String>,
    pub software: Vec<String>,
    pub hardware: Vec<String>,
    pub version_kind: String,
    pub version_label: String,
    pub version_date: String,
    pub description_short: String,
    pub description_long: String,
    pub document_artwork_sheet: String,
    pub document_presentation: String,
    pub document_technical_description: String,
    pub website_publish: bool,
    pub website_slug: String,
    pub website_visibility: String,
    pub relations_revised_by: Vec<String>,
    pub relations_related: Vec<String>,
    pub rights_copyright: String,
    pub rights_license: String,
}

/// Summary row for the overview screen.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub dir_name: String,
    pub id: String,
    pub title: String,
    pub year: String,
    pub project_type: String,
    pub status: String,
    pub documentation_level: String,
    pub document_artwork_sheet: String,
    pub document_presentation: String,
    pub document_technical_description: String,
    pub path: String,
    /// Set when manifest.toml is missing or unparseable. The project is still
    /// listed so a broken manifest never hides a project.
    pub manifest_error: Option<String>,
    pub warnings: Vec<String>,
}

/// Full detail for the project screen.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    pub dir_name: String,
    pub path: String,
    pub fields: ManifestFields,
    pub raw: String,
    pub warnings: Vec<String>,
    pub field_errors: BTreeMap<String, String>,
}

/// Result of a save attempt.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub saved: bool,
    pub dir_name: String,
    pub renamed: bool,
    pub raw: String,
    pub warnings: Vec<String>,
    pub field_errors: BTreeMap<String, String>,
}

// ── Reading ──────────────────────────────────────────────────────────

fn get_str(doc: &DocumentMut, key: &str) -> Option<String> {
    doc.get(key).and_then(|i| i.as_str()).map(|s| s.to_string())
}

fn get_i64(doc: &DocumentMut, key: &str) -> Option<i64> {
    doc.get(key).and_then(|i| i.as_integer())
}

fn get_str_array(doc: &DocumentMut, key: &str) -> Option<Vec<String>> {
    doc.get(key).and_then(|i| i.as_array()).map(|a| {
        a.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    })
}

fn table<'a>(doc: &'a DocumentMut, key: &str) -> Option<&'a Table> {
    doc.get(key).and_then(|i| i.as_table())
}

fn get_table_str(doc: &DocumentMut, tkey: &str, key: &str) -> Option<String> {
    table(doc, tkey)
        .and_then(|t| t.get(key))
        .and_then(|i| i.as_str())
        .map(|s| s.to_string())
}

fn get_table_bool(doc: &DocumentMut, tkey: &str, key: &str) -> Option<bool> {
    table(doc, tkey)
        .and_then(|t| t.get(key))
        .and_then(|i| i.as_bool())
}

fn get_table_str_array(doc: &DocumentMut, tkey: &str, key: &str) -> Option<Vec<String>> {
    table(doc, tkey)
        .and_then(|t| t.get(key))
        .and_then(|i| i.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
}

/// Parse known fields out of a manifest document.
///
/// Missing keys yield empty/default values and a warning each, so the UI can
/// show what is absent without blocking the edit.
pub fn fields_from_document(doc: &DocumentMut) -> (ManifestFields, Vec<String>) {
    let mut warnings = Vec::new();

    fn missing(warnings: &mut Vec<String>, key: &str) {
        warnings.push(format!("Missing field in manifest: {key}"));
    }

    macro_rules! get_or_missing {
        ($expr:expr, $key:expr) => {
            match $expr {
                Some(v) => v,
                None => {
                    missing(&mut warnings, $key);
                    Default::default()
                }
            }
        };
    }

    let id: String = get_or_missing!(get_str(doc, "id"), "id");
    let title: String = get_or_missing!(get_str(doc, "title"), "title");
    let year: String = match get_i64(doc, "year") {
        Some(v) => v.to_string(),
        None => {
            if doc.contains_key("year") {
                warnings.push("Field `year` is not an integer".to_string());
            } else {
                missing(&mut warnings, "year");
            }
            String::new()
        }
    };
    let project_type: String = get_or_missing!(get_str(doc, "type"), "type");
    let status: String = get_or_missing!(get_str(doc, "status"), "status");
    let documentation_level: String =
        get_or_missing!(get_str(doc, "documentation_level"), "documentation_level");

    let keywords: Vec<String> = get_or_missing!(get_str_array(doc, "keywords"), "keywords");
    let software: Vec<String> = get_or_missing!(get_str_array(doc, "software"), "software");
    let hardware: Vec<String> = get_or_missing!(get_str_array(doc, "hardware"), "hardware");

    macro_rules! table_field {
        ($getter:expr, $tkey:expr, $key:expr) => {{
            let dotted = concat!($tkey, ".", $key);
            match $getter(doc, $tkey, $key) {
                Some(v) => v,
                None => {
                    missing(&mut warnings, dotted);
                    Default::default()
                }
            }
        }};
    }

    let version_kind: String = table_field!(get_table_str, "version", "kind");
    let version_label: String = table_field!(get_table_str, "version", "label");
    let version_date: String = table_field!(get_table_str, "version", "date");
    let description_short: String = table_field!(get_table_str, "description", "short");
    let description_long: String = table_field!(get_table_str, "description", "long");
    let document_artwork_sheet: String = table_field!(get_table_str, "documents", "artwork_sheet");
    let document_presentation: String = table_field!(get_table_str, "documents", "presentation");
    let document_technical_description: String =
        table_field!(get_table_str, "documents", "technical_description");
    let website_publish: bool = table_field!(get_table_bool, "website", "publish");
    let website_slug: String = table_field!(get_table_str, "website", "slug");
    let website_visibility: String = table_field!(get_table_str, "website", "visibility");
    let relations_revised_by: Vec<String> =
        table_field!(get_table_str_array, "relations", "revised_by");
    let relations_related: Vec<String> = table_field!(get_table_str_array, "relations", "related");
    let rights_copyright: String = table_field!(get_table_str, "rights", "copyright");
    let rights_license: String = table_field!(get_table_str, "rights", "license");

    let fields = ManifestFields {
        id,
        title,
        year,
        project_type,
        status,
        documentation_level,
        keywords,
        software,
        hardware,
        version_kind,
        version_label,
        version_date,
        description_short,
        description_long,
        document_artwork_sheet,
        document_presentation,
        document_technical_description,
        website_publish,
        website_slug,
        website_visibility,
        relations_revised_by,
        relations_related,
        rights_copyright,
        rights_license,
    };

    (fields, warnings)
}

// ── Writing ──────────────────────────────────────────────────────────

/// Set a root-level string value, preserving the existing trailing comment.
///
/// Values that are already equal are not touched, so their original
/// encoding (basic / literal / multiline strings) survives a save.
fn set_str(doc: &mut DocumentMut, key: &str, val: &str) {
    if doc.get(key).and_then(|i| i.as_str()) == Some(val) {
        return;
    }
    set_value(doc, key, Value::from(val));
}

fn set_value(doc: &mut DocumentMut, key: &str, new: Value) {
    match doc.get_mut(key) {
        Some(item) => match item.as_value_mut() {
            Some(old) => {
                let decor = old.decor().clone();
                let mut new = new;
                *new.decor_mut() = decor;
                *old = new;
            }
            None => *item = Item::Value(new),
        },
        None => {
            doc.insert(key, Item::Value(new));
        }
    }
}

fn set_str_array(doc: &mut DocumentMut, key: &str, items: &[String]) {
    if get_str_array(doc, key).as_deref() == Some(items) {
        return;
    }
    let mut arr = Array::new();
    for item in items {
        arr.push(item.as_str());
    }
    set_value(doc, key, Value::Array(arr));
}

fn ensure_table<'a>(doc: &'a mut DocumentMut, key: &str) -> &'a mut Table {
    if !doc.contains_key(key) {
        doc.insert(key, Item::Table(Table::new()));
    }
    doc.get_mut(key)
        .and_then(|i| i.as_table_mut())
        .expect("ensured table exists")
}

fn set_table_str(doc: &mut DocumentMut, tkey: &str, key: &str, val: &str) {
    if get_table_str(doc, tkey, key).as_deref() == Some(val) {
        return;
    }
    let t = ensure_table(doc, tkey);
    set_in_table(t, key, Value::from(val));
}

fn set_table_bool(doc: &mut DocumentMut, tkey: &str, key: &str, val: bool) {
    if get_table_bool(doc, tkey, key) == Some(val) {
        return;
    }
    let t = ensure_table(doc, tkey);
    set_in_table(t, key, Value::from(val));
}

fn set_table_str_array(doc: &mut DocumentMut, tkey: &str, key: &str, items: &[String]) {
    if get_table_str_array(doc, tkey, key).as_deref() == Some(items) {
        return;
    }
    let mut arr = Array::new();
    for item in items {
        arr.push(item.as_str());
    }
    let t = ensure_table(doc, tkey);
    set_in_table(t, key, Value::Array(arr));
}

fn set_in_table(table: &mut Table, key: &str, new: Value) {
    match table.get_mut(key) {
        Some(item) => match item.as_value_mut() {
            Some(old) => {
                let decor = old.decor().clone();
                let mut new = new;
                *new.decor_mut() = decor;
                *old = new;
            }
            None => *item = Item::Value(new),
        },
        None => {
            table.insert(key, Item::Value(new));
        }
    }
}

/// Validate + convert raw field input, returning per-field errors.
///
/// `year` must be an integer when present. Unknown enum values are not
/// rejected here — they are flagged by the caller as warnings and preserved
/// in the manifest.
pub fn validate(fields: &ManifestFields) -> BTreeMap<String, String> {
    let mut errors = BTreeMap::new();

    if !fields.year.trim().is_empty() && fields.year.trim().parse::<i64>().is_err() {
        errors.insert(
            "year".to_string(),
            format!("“{}” is not a whole number", fields.year),
        );
    }

    if let Some(msg) = unsafe_id_reason(&fields.id) {
        errors.insert("id".to_string(), msg);
    }

    errors
}

/// Why an archive ID cannot be used as a directory name, if any.
///
/// The repository contains no ID pattern rules beyond the examples in
/// `manifest.toml`, so only filesystem-safety constraints are enforced here.
pub fn unsafe_id_reason(id: &str) -> Option<String> {
    if id.trim().is_empty() {
        return Some("Archive ID must not be empty".to_string());
    }
    if id != id.trim() {
        return Some("Archive ID must not start or end with whitespace".to_string());
    }
    if id == "." || id == ".." {
        return Some("Archive ID must not be “.” or “..”".to_string());
    }
    if id.contains('/') || id.contains('\\') {
        return Some("Archive ID must not contain path separators".to_string());
    }
    if id.starts_with('.') {
        return Some("Archive ID must not start with a dot".to_string());
    }
    if id.chars().any(|c| c.is_control()) {
        return Some("Archive ID must not contain control characters".to_string());
    }
    None
}

/// Apply validated fields onto an existing document, preserving unknown
/// fields and comments. Returns warnings for values that are kept but do not
/// match the documented value set.
pub fn apply_fields(doc: &mut DocumentMut, fields: &ManifestFields) -> Vec<String> {
    let mut warnings = Vec::new();

    warn_unknown(
        &mut warnings,
        "status",
        &fields.status,
        &["draft", "wip", "presentable", "archived"],
    );
    warn_unknown(
        &mut warnings,
        "documentation_level",
        &fields.documentation_level,
        &["none", "stub", "partial", "complete"],
    );
    warn_unknown(
        &mut warnings,
        "documents.artwork_sheet",
        &fields.document_artwork_sheet,
        &["missing", "wip", "ready"],
    );
    warn_unknown(
        &mut warnings,
        "documents.presentation",
        &fields.document_presentation,
        &["missing", "wip", "ready"],
    );
    warn_unknown(
        &mut warnings,
        "documents.technical_description",
        &fields.document_technical_description,
        &["missing", "wip", "ready"],
    );
    warn_unknown(
        &mut warnings,
        "website.visibility",
        &fields.website_visibility,
        &["public", "unlisted", "private"],
    );
    warn_unknown(
        &mut warnings,
        "version.kind",
        &fields.version_kind,
        &["original", "revision", "adaptation"],
    );

    let year = fields.year.trim();
    if year.is_empty() {
        warnings.push("Field `year` left unchanged (empty input)".to_string());
    } else if let Ok(y) = year.parse::<i64>() {
        if get_i64(doc, "year") != Some(y) {
            set_value(doc, "year", Value::from(y));
        }
    }

    set_str(doc, "id", &fields.id);
    set_str(doc, "title", &fields.title);
    set_str(doc, "type", &fields.project_type);
    set_str(doc, "status", &fields.status);
    set_str(doc, "documentation_level", &fields.documentation_level);
    set_str_array(doc, "keywords", &fields.keywords);
    set_str_array(doc, "software", &fields.software);
    set_str_array(doc, "hardware", &fields.hardware);

    set_table_str(doc, "version", "kind", &fields.version_kind);
    set_table_str(doc, "version", "label", &fields.version_label);
    set_table_str(doc, "version", "date", &fields.version_date);

    set_table_str(doc, "description", "short", &fields.description_short);
    set_table_str(doc, "description", "long", &fields.description_long);

    set_table_str(
        doc,
        "documents",
        "artwork_sheet",
        &fields.document_artwork_sheet,
    );
    set_table_str(
        doc,
        "documents",
        "presentation",
        &fields.document_presentation,
    );
    set_table_str(
        doc,
        "documents",
        "technical_description",
        &fields.document_technical_description,
    );

    set_table_bool(doc, "website", "publish", fields.website_publish);
    set_table_str(doc, "website", "slug", &fields.website_slug);
    set_table_str(doc, "website", "visibility", &fields.website_visibility);

    set_table_str_array(doc, "relations", "revised_by", &fields.relations_revised_by);
    set_table_str_array(doc, "relations", "related", &fields.relations_related);

    set_table_str(doc, "rights", "copyright", &fields.rights_copyright);
    set_table_str(doc, "rights", "license", &fields.rights_license);

    warnings
}

fn warn_unknown(warnings: &mut Vec<String>, field: &str, value: &str, known: &[&str]) {
    let v = value.trim();
    if !v.is_empty() && !known.contains(&v) {
        warnings.push(format!(
            "Field `{field}` value “{v}” is not one of: {} (kept as-is)",
            known.join(", ")
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"# Project Manifest
id = "MI-2026-TE"
title = "Test"
year = 2026
type = "Software"                   # e.g., Audio-Visual-Computation
status = "draft"                    # draft | wip | presentable | archived
documentation_level = "none"        # none | stub | partial | complete

keywords = []
software = []
hardware = []
custom_field = "must survive"       # unknown root field

[version]
kind = "original"                   # original | revision | adaptation
label = "Initial Version"
date = "2026-06"                    # YYYY or YYYY-MM

[description]
short = ""
long = """
multi
line
"""

[documents]
artwork_sheet = "missing"           # missing | wip | ready
presentation = "missing"
technical_description = "missing"

[website]
publish = false
slug = ""
visibility = "private"              # public | unlisted | private

[relations]
revised_by = []
related = []

[rights]
copyright = "© monointerferenz"
license = "all rights reserved"
"#;

    #[test]
    fn reads_all_known_fields() {
        let doc: DocumentMut = SAMPLE.parse().unwrap();
        let (f, warnings) = fields_from_document(&doc);
        assert_eq!(f.id, "MI-2026-TE");
        assert_eq!(f.year, "2026");
        assert_eq!(f.status, "draft");
        assert!(!f.website_publish);
        assert_eq!(f.rights_copyright, "© monointerferenz");
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn roundtrip_preserves_unknown_fields_and_comments() {
        let doc: DocumentMut = SAMPLE.parse().unwrap();
        let (mut f, _) = fields_from_document(&doc);
        f.status = "wip".into();
        f.title = "Renamed".into();

        let mut doc = doc;
        apply_fields(&mut doc, &f);
        let out = doc.to_string();

        assert!(
            out.contains("custom_field = \"must survive\""),
            "unknown field lost:\n{out}"
        );
        assert!(out.contains("# unknown root field"), "comment lost:\n{out}");
        assert!(
            out.contains("status = \"wip\""),
            "status not updated:\n{out}"
        );
        assert!(
            out.contains("# draft | wip | presentable | archived"),
            "trailing comment lost:\n{out}"
        );
        assert!(
            out.contains("multi\nline"),
            "multiline string broken:\n{out}"
        );
    }

    #[test]
    fn missing_fields_produce_warnings_not_errors() {
        let doc: DocumentMut = "id = \"MI-2026-TE\"\n".parse().unwrap();
        let (f, warnings) = fields_from_document(&doc);
        assert_eq!(f.id, "MI-2026-TE");
        assert!(warnings.iter().any(|w| w.contains("title")));
    }

    #[test]
    fn id_safety_rules() {
        assert!(unsafe_id_reason("MI-2026-TE").is_none());
        assert!(unsafe_id_reason("").is_some());
        assert!(unsafe_id_reason("../evil").is_some());
        assert!(unsafe_id_reason("a/b").is_some());
        assert!(unsafe_id_reason(".hidden").is_some());
    }

    #[test]
    fn invalid_year_is_reported_per_field() {
        let f = ManifestFields {
            year: "twenty".into(),
            ..Default::default()
        };
        let errors = validate(&f);
        assert!(errors.contains_key("year"));
    }

    /// The real template shipped in DEV-RESOURCES is the data contract.
    #[test]
    fn real_template_manifest_roundtrip() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../DEV-RESOURCES/Project-Template/MI-2026-TE/manifest.toml");
        let raw = std::fs::read_to_string(&path).expect("template manifest readable");
        let doc: DocumentMut = raw.parse().expect("template manifest parses");
        let (mut f, warnings) = fields_from_document(&doc);
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert_eq!(f.id, "MI-2026-XXX");
        assert_eq!(f.year, "2026");
        assert_eq!(f.rights_license, "all rights reserved");
        assert!(!f.website_publish);

        f.status = "wip".into();
        f.title = "Round Trip".into();
        f.keywords = vec!["test".into()];

        let mut doc = doc;
        apply_fields(&mut doc, &f);
        let out = doc.to_string();

        // Comments and untouched encodings survive.
        assert!(
            out.contains("# Project Manifest"),
            "header comment lost:\n{out}"
        );
        assert!(
            out.contains("# draft | wip | presentable | archived"),
            "status comment lost:\n{out}"
        );
        assert!(
            out.contains("# e.g., Audio-Visual-Computation"),
            "type comment lost:\n{out}"
        );
        assert!(
            out.contains("long = \"\"\""),
            "multiline style lost:\n{out}"
        );

        // Edits are applied.
        assert!(
            out.contains("status = \"wip\""),
            "status not updated:\n{out}"
        );
        assert!(
            out.contains("title = \"Round Trip\""),
            "title not updated:\n{out}"
        );
        assert!(
            out.contains("keywords = [\"test\"]"),
            "keywords not updated:\n{out}"
        );

        // The result parses back to the same values with no warnings.
        let reparsed: DocumentMut = out.parse().expect("roundtrip parses");
        let (f2, w2) = fields_from_document(&reparsed);
        assert!(w2.is_empty(), "unexpected warnings: {w2:?}");
        assert_eq!(f2.status, "wip");
        assert_eq!(f2.title, "Round Trip");
        assert_eq!(f2.year, "2026");
        assert_eq!(f2.keywords, vec!["test".to_string()]);
        assert_eq!(f2.document_artwork_sheet, "missing");
    }
}

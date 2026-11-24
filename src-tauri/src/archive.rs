//! Archive scanning and project manifest operations.
//!
//! An archive is a directory of project directories; each project directory
//! is named by its archive ID and contains a `manifest.toml`.

use crate::manifest::{self, ProjectDetail, ProjectSummary, SaveResult};
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::DocumentMut;

#[derive(Debug)]
pub struct AppError(pub String);

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

/// List all project directories in an archive.
///
/// Directories without a readable manifest are still returned, flagged with
/// `manifest_error`, so a broken manifest never hides a project.
pub fn list_projects(archive: &Path) -> Result<Vec<ProjectSummary>> {
    if !archive.is_dir() {
        return Err(AppError(format!(
            "Archive folder not found or not a directory: {}",
            archive.display()
        )));
    }

    let mut entries: Vec<PathBuf> = fs::read_dir(archive)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .filter(|p| {
            !p.file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(false)
        })
        .collect();
    entries.sort();

    let mut out = Vec::new();
    for dir in entries {
        let dir_name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let manifest_path = dir.join("manifest.toml");

        let summary = if !manifest_path.exists() {
            ProjectSummary {
                dir_name: dir_name.clone(),
                id: dir_name.clone(),
                title: String::new(),
                year: String::new(),
                project_type: String::new(),
                status: String::new(),
                documentation_level: String::new(),
                document_artwork_sheet: String::new(),
                document_presentation: String::new(),
                document_technical_description: String::new(),
                path: dir.display().to_string(),
                manifest_error: Some("manifest.toml is missing".to_string()),
                warnings: Vec::new(),
            }
        } else {
            match read_manifest(&manifest_path) {
                Ok((fields, warnings)) => ProjectSummary {
                    dir_name: dir_name.clone(),
                    id: fields.id,
                    title: fields.title,
                    year: fields.year,
                    project_type: fields.project_type,
                    status: fields.status,
                    documentation_level: fields.documentation_level,
                    document_artwork_sheet: fields.document_artwork_sheet,
                    document_presentation: fields.document_presentation,
                    document_technical_description: fields.document_technical_description,
                    path: dir.display().to_string(),
                    manifest_error: None,
                    warnings,
                },
                Err(e) => ProjectSummary {
                    dir_name: dir_name.clone(),
                    id: dir_name.clone(),
                    title: String::new(),
                    year: String::new(),
                    project_type: String::new(),
                    status: String::new(),
                    documentation_level: String::new(),
                    document_artwork_sheet: String::new(),
                    document_presentation: String::new(),
                    document_technical_description: String::new(),
                    path: dir.display().to_string(),
                    manifest_error: Some(e.0),
                    warnings: Vec::new(),
                },
            }
        };
        out.push(summary);
    }

    Ok(out)
}

fn read_manifest(path: &Path) -> Result<(manifest::ManifestFields, Vec<String>)> {
    let raw = fs::read_to_string(path)?;
    let doc: DocumentMut = raw
        .parse()
        .map_err(|e: toml_edit::TomlError| AppError(format!("Invalid manifest.toml: {e}")))?;
    Ok(manifest::fields_from_document(&doc))
}

/// Load a single project for the project screen.
pub fn get_project(archive: &Path, dir_name: &str) -> Result<ProjectDetail> {
    let dir = safe_project_dir(archive, dir_name)?;
    let manifest_path = dir.join("manifest.toml");
    if !manifest_path.exists() {
        return Err(AppError(format!(
            "manifest.toml is missing in {}",
            dir.display()
        )));
    }
    let raw = fs::read_to_string(&manifest_path)?;
    let doc: DocumentMut = raw
        .parse()
        .map_err(|e: toml_edit::TomlError| AppError(format!("Invalid manifest.toml: {e}")))?;
    let (fields, warnings) = manifest::fields_from_document(&doc);

    let mut field_errors = std::collections::BTreeMap::new();
    if let Some(reason) = manifest::unsafe_id_reason(&fields.id) {
        field_errors.insert("id".to_string(), reason);
    }

    Ok(ProjectDetail {
        dir_name: dir_name.to_string(),
        path: dir.display().to_string(),
        fields,
        raw,
        warnings,
        field_errors,
    })
}

/// Save edited fields to a project's manifest.
///
/// Unknown fields and comments are preserved (format-preserving TOML edit).
/// If the archive ID changed, the project directory is renamed to match —
/// the ID is the directory name.
pub fn save_project(
    archive: &Path,
    dir_name: &str,
    fields: &manifest::ManifestFields,
) -> Result<SaveResult> {
    let mut field_errors = manifest::validate(fields);

    // The directory we edit is the current one; the ID may rename it.
    let dir = safe_project_dir(archive, dir_name)?;
    let manifest_path = dir.join("manifest.toml");

    if !manifest_path.exists() {
        return Err(AppError(format!(
            "manifest.toml is missing in {}",
            dir.display()
        )));
    }

    let raw = fs::read_to_string(&manifest_path)?;
    let mut doc: DocumentMut = raw
        .parse()
        .map_err(|e: toml_edit::TomlError| AppError(format!("Invalid manifest.toml: {e}")))?;

    // Validate ID against the *target* name before touching anything.
    let new_id = fields.id.clone();
    let mut rename_to: Option<PathBuf> = None;
    if new_id != dir_name {
        if let Some(reason) = manifest::unsafe_id_reason(&new_id) {
            field_errors.insert("id".to_string(), reason);
        } else {
            let target = archive.join(&new_id);
            if target.exists() {
                field_errors.insert(
                    "id".to_string(),
                    format!("A project directory named “{new_id}” already exists"),
                );
            } else {
                rename_to = Some(target);
            }
        }
    }

    if !field_errors.is_empty() {
        return Ok(SaveResult {
            saved: false,
            dir_name: dir_name.to_string(),
            renamed: false,
            raw,
            warnings: Vec::new(),
            field_errors,
        });
    }

    let mut warnings = manifest::apply_fields(&mut doc, fields);
    let new_raw = doc.to_string();

    // Write first, rename second: never leave a renamed directory with a
    // stale manifest. If the rename fails the manifest is already correct
    // for the old directory and the error is reported.
    fs::write(&manifest_path, &new_raw)?;

    let mut final_dir_name = dir_name.to_string();
    let mut renamed = false;
    if let Some(target) = rename_to {
        fs::rename(&dir, &target).map_err(|e| {
            AppError(format!(
                "Manifest saved, but renaming the project directory to “{new_id}” failed: {e}"
            ))
        })?;
        final_dir_name = new_id;
        renamed = true;
        warnings.push(format!("Project directory renamed to “{final_dir_name}”"));
    }

    Ok(SaveResult {
        saved: true,
        dir_name: final_dir_name,
        renamed,
        raw: new_raw,
        warnings,
        field_errors,
    })
}

/// Resolve a project directory inside the archive, rejecting path escapes.
fn safe_project_dir(archive: &Path, dir_name: &str) -> Result<PathBuf> {
    if let Some(reason) = manifest::unsafe_id_reason(dir_name) {
        return Err(AppError(format!(
            "Invalid project directory name: {reason}"
        )));
    }
    let dir = archive.join(dir_name);
    // Belt and braces: canonical path must stay within the archive.
    if let (Ok(canon_dir), Ok(canon_archive)) = (dir.canonicalize(), archive.canonicalize()) {
        if !canon_dir.starts_with(&canon_archive) {
            return Err(AppError(
                "Project path escapes the archive folder".to_string(),
            ));
        }
    }
    if !dir.is_dir() {
        return Err(AppError(format!(
            "Project directory not found: {}",
            dir.display()
        )));
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_archive(name: &str) -> PathBuf {
        let mut dir = env::temp_dir();
        dir.push(format!("mono-archive-test-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_sample(archive: &Path, dir: &str) {
        let p = archive.join(dir);
        fs::create_dir_all(&p).unwrap();
        fs::write(
            p.join("manifest.toml"),
            "id = \"MI-2026-TE\"\ntitle = \"T\"\nyear = 2026\ntype = \"Software\"\nstatus = \"draft\"\ndocumentation_level = \"none\"\nkeywords = []\nsoftware = []\nhardware = []\n\n[version]\nkind = \"original\"\nlabel = \"L\"\ndate = \"2026-06\"\n\n[description]\nshort = \"\"\nlong = \"\"\"\"\"\"\n\n[documents]\nartwork_sheet = \"missing\"\npresentation = \"missing\"\ntechnical_description = \"missing\"\n\n[website]\npublish = false\nslug = \"\"\nvisibility = \"private\"\n\n[relations]\nrevised_by = []\nrelated = []\n\n[rights]\ncopyright = \"c\"\nlicense = \"l\"\n",
        )
        .unwrap();
    }

    #[test]
    fn lists_projects_and_flags_broken_manifests() {
        let archive = temp_archive("list");
        write_sample(&archive, "MI-2026-TE");
        let broken = archive.join("MI-2026-BROKEN");
        fs::create_dir_all(&broken).unwrap();
        fs::write(broken.join("manifest.toml"), "id = [broken").unwrap();

        let list = list_projects(&archive).unwrap();
        assert_eq!(list.len(), 2);
        let ok = list.iter().find(|p| p.dir_name == "MI-2026-TE").unwrap();
        assert!(ok.manifest_error.is_none());
        assert_eq!(ok.status, "draft");
        let broken = list
            .iter()
            .find(|p| p.dir_name == "MI-2026-BROKEN")
            .unwrap();
        assert!(broken.manifest_error.is_some());

        fs::remove_dir_all(&archive).ok();
    }

    #[test]
    fn save_rejects_path_escape_ids() {
        let archive = temp_archive("escape");
        write_sample(&archive, "MI-2026-TE");

        let fields = manifest::ManifestFields {
            id: "../evil".into(),
            ..Default::default()
        };
        let res = save_project(&archive, "MI-2026-TE", &fields).unwrap();
        assert!(!res.saved);
        assert!(res.field_errors.contains_key("id"));
        assert!(archive.join("MI-2026-TE").exists());

        fs::remove_dir_all(&archive).ok();
    }

    #[test]
    fn save_renames_directory_when_id_changes() {
        let archive = temp_archive("rename");
        write_sample(&archive, "MI-2026-TE");

        let detail = get_project(&archive, "MI-2026-TE").unwrap();
        let mut fields = detail.fields;
        fields.id = "MI-2026-NE".into();
        let res = save_project(&archive, "MI-2026-TE", &fields).unwrap();
        assert!(res.saved);
        assert!(res.renamed);
        assert_eq!(res.dir_name, "MI-2026-NE");
        assert!(!archive.join("MI-2026-TE").exists());
        assert!(archive.join("MI-2026-NE").join("manifest.toml").exists());

        fs::remove_dir_all(&archive).ok();
    }

    #[test]
    fn missing_archive_is_an_actionable_error() {
        let err = list_projects(Path::new("/nonexistent/archive/xyz")).unwrap_err();
        assert!(err.0.contains("not found"));
    }
}

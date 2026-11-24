mod archive;
mod engine;
mod manifest;
mod paths;
mod settings;

use archive::{AppError, Result};
use std::path::PathBuf;
use tauri::{Manager, State};

struct AppState {
    config_dir: PathBuf,
}

fn load_settings(state: &State<AppState>) -> settings::Settings {
    settings::load(&state.config_dir)
}

fn archive_path(state: &State<AppState>) -> Result<PathBuf> {
    let s = load_settings(state);
    if s.archive_path.trim().is_empty() {
        return Err(AppError(
            "No archive folder configured yet. Choose one in Settings.".to_string(),
        ));
    }
    Ok(PathBuf::from(s.archive_path))
}

// ── Settings ─────────────────────────────────────────────────────────

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<settings::Settings> {
    Ok(load_settings(&state))
}

#[tauri::command]
async fn save_settings(
    state: State<'_, AppState>,
    settings: settings::Settings,
) -> Result<settings::Settings> {
    settings::save(&state.config_dir, &settings)
        .map_err(|e| AppError(format!("Could not save settings: {e}")))?;
    Ok(settings.resolved())
}

// ── Projects ─────────────────────────────────────────────────────────

#[tauri::command]
async fn list_projects(state: State<'_, AppState>) -> Result<Vec<manifest::ProjectSummary>> {
    archive::list_projects(&archive_path(&state)?)
}

#[tauri::command]
async fn get_project(
    state: State<'_, AppState>,
    dir_name: String,
) -> Result<manifest::ProjectDetail> {
    archive::get_project(&archive_path(&state)?, &dir_name)
}

#[tauri::command]
async fn save_project(
    state: State<'_, AppState>,
    dir_name: String,
    fields: manifest::ManifestFields,
) -> Result<manifest::SaveResult> {
    archive::save_project(&archive_path(&state)?, &dir_name, &fields)
}

/// Create a new archive project by cloning the supplied template and
/// stamping it with the given archive ID and title.
#[tauri::command]
async fn create_project(
    state: State<'_, AppState>,
    id: String,
    title: String,
) -> Result<manifest::ProjectSummary> {
    let archive = archive_path(&state)?;
    let s = load_settings(&state);

    if let Some(reason) = manifest::unsafe_id_reason(&id) {
        return Err(AppError(reason));
    }
    let target = archive.join(&id);
    if target.exists() {
        return Err(AppError(format!(
            "A project directory named “{id}” already exists."
        )));
    }

    let template = PathBuf::from(&s.template_path);
    if !template.is_dir() {
        return Err(AppError(format!(
            "Project template not found at {}. Set the template folder in Settings.",
            template.display()
        )));
    }

    clone_dir(&template, &target)?;

    // Stamp the manifest with the new ID and title.
    let manifest_path = target.join("manifest.toml");
    let raw = std::fs::read_to_string(&manifest_path)
        .map_err(|e| AppError(format!("Could not read the template manifest: {e}")))?;
    let mut doc: toml_edit::DocumentMut = raw
        .parse()
        .map_err(|e: toml_edit::TomlError| AppError(format!("Invalid template manifest: {e}")))?;
    doc["id"] = toml_edit::value(id.as_str());
    doc["title"] = toml_edit::value(title.as_str());
    std::fs::write(&manifest_path, doc.to_string())
        .map_err(|e| AppError(format!("Could not write the new manifest: {e}")))?;

    archive::get_project(&archive, &id).map(|d| manifest::ProjectSummary {
        dir_name: d.dir_name,
        id: d.fields.id,
        title: d.fields.title,
        year: d.fields.year,
        project_type: d.fields.project_type,
        status: d.fields.status,
        documentation_level: d.fields.documentation_level,
        document_artwork_sheet: d.fields.document_artwork_sheet,
        document_presentation: d.fields.document_presentation,
        document_technical_description: d.fields.document_technical_description,
        path: d.path,
        manifest_error: None,
        warnings: d.warnings,
    })
}

fn clone_dir(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_string_lossy() == ".DS_Store" {
            continue;
        }
        let from = entry.path();
        let to = dst.join(&name);
        if from.is_dir() {
            clone_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

// ── Presentation tool ────────────────────────────────────────────────

#[tauri::command]
async fn build_documents(
    state: State<'_, AppState>,
    dir_name: String,
    target: String,
) -> Result<engine::BuildOutcome> {
    let archive = archive_path(&state)?;
    let s = load_settings(&state);

    let target = engine::BuildTarget::parse(&target)
        .ok_or_else(|| AppError(format!("Unknown build target: {target}")))?;

    let project = archive.join(&dir_name);
    if !project.is_dir() {
        return Err(AppError(format!(
            "Project directory not found: {}",
            project.display()
        )));
    }
    if let Some(reason) = manifest::unsafe_id_reason(&dir_name) {
        return Err(AppError(format!(
            "Invalid project directory name: {reason}"
        )));
    }

    let engine_root = PathBuf::from(&s.engine_root);
    if !engine_root.is_dir() {
        return Err(AppError(format!(
            "Engine root not found at {}. Set it in Settings.",
            engine_root.display()
        )));
    }
    let binary = PathBuf::from(&s.engine_binary);

    engine::build(&project, &dir_name, &engine_root, &binary, target)
}

/// Open a folder in Finder (used for the archive and exports folders).
#[tauri::command]
fn reveal_in_finder(path: String) -> Result<()> {
    std::process::Command::new("open")
        .arg(&path)
        .spawn()
        .map_err(|e| AppError(format!("Could not open {path}: {e}")))?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_dir = app
                .path()
                .app_config_dir()
                .map_err(|e| format!("Could not resolve config directory: {e}"))?;
            app.manage(AppState { config_dir });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            list_projects,
            get_project,
            save_project,
            create_project,
            build_documents,
            reveal_in_finder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running mono-archive");
}

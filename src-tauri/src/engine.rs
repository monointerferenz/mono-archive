//! Integration with the supplied monointerferenz templating engine.
//!
//! The engine builds documentation for projects located at
//! `<root>/projects/<slug>/…`, while archives keep projects in their own
//! folder. To keep the supplied tool untouched, each build runs against a
//! temporary *staging root*:
//!
//! ```text
//! staging/
//! ├── themes/      → engine themes       (symlink)
//! ├── scripts/     → engine scripts      (symlink)
//! ├── dist/        → <project>/exports/  (symlink — outputs land in the archive)
//! └── projects/<id>/  → symlinks to the archive project's files
//! ```
//!
//! The engine is invoked with `MI_ROOT=<staging>` and the working directory
//! set to the staging root (its `dist/…` output paths are relative to the
//! cwd). Original sources, the engine source and its binary are never
//! modified.

use crate::archive::AppError;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildTarget {
    Html,
    Pdf,
    Slides,
    All,
}

impl BuildTarget {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "html" => Some(Self::Html),
            "pdf" => Some(Self::Pdf),
            "slides" => Some(Self::Slides),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Html => "HTML",
            Self::Pdf => "PDF",
            Self::Slides => "Slides",
            Self::All => "All exports",
        }
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildOutcome {
    pub success: bool,
    pub target: String,
    pub output: String,
    /// Files now present in the project's `exports/` directory.
    pub exports: Vec<String>,
    pub exports_dir: String,
}

/// Guard that removes the staging root when dropped.
struct Staging(PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Candidate locations of the engine's document sources inside an archive
/// project, in priority order.
fn source_candidates(file_name: &str) -> &'static [&'static str] {
    match file_name {
        "project-log.md" => &["project-log.md", "documentation/project-log.md"],
        "artwork-sheet.md" => &["artwork-sheet.md", "documentation/artwork-sheet.md"],
        "presentation.md" => &[
            "presentation.md",
            "presennataion/presentation.md",
            "presentation/presentation.md",
            "documentation/presentation.md",
        ],
        _ => &[],
    }
}

fn find_source(project: &Path, file_name: &str) -> Option<PathBuf> {
    source_candidates(file_name)
        .iter()
        .map(|rel| project.join(rel))
        .find(|p| p.is_file())
}

/// Build a staging root for one project. Every existing file and directory
/// of the archive project is linked in so relative references inside the
/// Markdown keep resolving; the engine's three document names are linked
/// from their archive locations.
fn prepare_staging(
    project: &Path,
    engine_root: &Path,
    project_id: &str,
) -> Result<(Staging, PathBuf), AppError> {
    let staging_dir = project.join("build").join(".mi-staging");
    let _ = fs::remove_dir_all(&staging_dir);
    fs::create_dir_all(&staging_dir)
        .map_err(|e| AppError(format!("Could not create staging directory: {e}")))?;

    let stage_project = staging_dir.join("projects").join(project_id);
    fs::create_dir_all(&stage_project)
        .map_err(|e| AppError(format!("Could not create staging project directory: {e}")))?;

    // 1. Link the archive project's own files/dirs into the staging project.
    for entry in fs::read_dir(project)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy().to_string();
        if name_str == ".DS_Store" || name_str == "build" {
            continue;
        }
        symlink_or_skip(&entry.path(), &stage_project.join(&name))?;
    }

    // 2. Link the engine's document sources under the names it expects.
    for file_name in ["project-log.md", "artwork-sheet.md", "presentation.md"] {
        let target = stage_project.join(file_name);
        if target.exists() {
            continue;
        }
        if let Some(source) = find_source(project, file_name) {
            symlink_or_skip(&source, &target)?;
        }
    }

    // 3. Engine resources.
    let themes = engine_root.join("themes");
    if !themes.is_dir() {
        return Err(AppError(format!(
            "Engine themes folder not found: {}",
            themes.display()
        )));
    }
    symlink_or_skip(&themes, &staging_dir.join("themes"))?;
    symlink_or_skip(&engine_root.join("scripts"), &staging_dir.join("scripts"))?;
    for extra in ["mi-options.json", "templates"] {
        let source = engine_root.join(extra);
        if source.exists() {
            symlink_or_skip(&source, &staging_dir.join(extra))?;
        }
    }

    // 4. Outputs go to the archive project's exports/ directory.
    let exports = project.join("exports");
    fs::create_dir_all(&exports)
        .map_err(|e| AppError(format!("Could not create exports directory: {e}")))?;
    symlink_or_skip(&exports, &staging_dir.join("dist"))?;

    Ok((Staging(staging_dir.clone()), staging_dir))
}

fn symlink_or_skip(source: &Path, target: &Path) -> Result<(), AppError> {
    if target.exists() {
        return Ok(());
    }
    std::os::unix::fs::symlink(source, target)
        .map_err(|e| AppError(format!("Could not link {} : {e}", source.display())))
}

/// GUI apps on macOS start with a minimal PATH; include the usual package
/// manager locations so `pandoc`, `marp` and `xelatex` are found.
fn augmented_path() -> String {
    let mut parts: Vec<String> = vec![
        "/opt/homebrew/bin".into(),
        "/usr/local/bin".into(),
        "/Library/TeX/texbin".into(),
        "/usr/bin".into(),
        "/bin".into(),
        "/usr/sbin".into(),
        "/sbin".into(),
    ];
    if let Ok(path) = std::env::var("PATH") {
        parts.push(path);
    }
    if let Ok(home) = std::env::var("HOME") {
        parts.push(format!("{home}/.local/bin"));
    }
    parts.join(":")
}

fn which_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .env("PATH", augmented_path())
        .output()
        .is_ok()
}

/// Run one engine subcommand inside the staging root.
fn run_engine_step(
    binary: &Path,
    staging_root: &Path,
    project_id: &str,
    args: &[&str],
    label: &str,
) -> Result<String, AppError> {
    let output = Command::new(binary)
        .args(args)
        .arg("--project")
        .arg(project_id)
        .env("MI_ROOT", staging_root)
        .env("PATH", augmented_path())
        .current_dir(staging_root)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError(format!(
                    "Engine binary not found at {}. \
                     Build it with `cargo build --release` inside \
                     DEV-RESOURCES/monointerferenz-templating-engine, \
                     or set the correct path in Settings.",
                    binary.display()
                ))
            } else {
                AppError(format!("Could not run the engine: {e}"))
            }
        })?;

    let mut text = String::new();
    text.push_str(&format!("── {label} ──\n"));
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    text.push_str(&String::from_utf8_lossy(&output.stderr));

    if !output.status.success() {
        return Err(AppError(format!(
            "{label} failed (exit code {}):\n{text}",
            output.status.code().unwrap_or(-1)
        )));
    }
    Ok(text)
}

/// Build documentation for an archive project via the supplied engine.
pub fn build(
    project: &Path,
    project_id: &str,
    engine_root: &Path,
    binary: &Path,
    target: BuildTarget,
) -> Result<BuildOutcome, AppError> {
    if !binary.is_file() {
        return Err(AppError(format!(
            "Engine binary not found at {}. \
             Build it with `cargo build --release` inside \
             DEV-RESOURCES/monointerferenz-templating-engine, \
             or set the correct path in Settings.",
            binary.display()
        )));
    }

    // Which document sources exist in this project?
    let has_log = find_source(project, "project-log.md").is_some();
    let has_artwork = find_source(project, "artwork-sheet.md").is_some();
    let has_presentation = find_source(project, "presentation.md").is_some();

    if !has_log && !has_artwork && !has_presentation {
        return Err(AppError(format!(
            "No documentation sources found in {}.\n\
             Looked for project-log.md, artwork-sheet.md and presentation.md \
             (in the project root, documentation/ and presennataion/).",
            project.display()
        )));
    }

    // External tool pre-checks with actionable messages.
    let needs_pandoc = matches!(
        target,
        BuildTarget::Html | BuildTarget::Pdf | BuildTarget::All
    ) && (has_log || has_artwork);
    let needs_marp = matches!(target, BuildTarget::Slides | BuildTarget::All) && has_presentation;
    if needs_pandoc && !which_exists("pandoc") {
        return Err(AppError(
            "“pandoc” was not found on PATH. Install it with `brew install pandoc` and try again."
                .to_string(),
        ));
    }
    if needs_marp && !which_exists("marp") {
        // The engine falls back to `npx marp`; accept either.
        if !which_exists("npx") {
            return Err(AppError(
                "Neither “marp” nor “npx” was found on PATH. \
                 Install Marp (`npm install -g @marp-team/marp-cli`) and try again."
                    .to_string(),
            ));
        }
    }
    if matches!(target, BuildTarget::Pdf | BuildTarget::All)
        && (has_log || has_artwork)
        && !which_exists("xelatex")
    {
        return Err(AppError(
            "“xelatex” was not found on PATH (required for PDF export). \
             Install a LaTeX distribution, e.g. `brew install --cask mactex-no-gui`."
                .to_string(),
        ));
    }

    let (staging, staging_root) = prepare_staging(project, engine_root, project_id)?;

    let mut log = String::new();
    let mut failures: Vec<String> = Vec::new();

    // html / pdf build the available document kinds; slides needs the
    // presentation source. Missing sources are skipped instead of failing
    // the whole run — the engine itself reports unreadable Markdown.
    let kind = match (has_log, has_artwork) {
        (true, true) => "all",
        (true, false) => "log",
        (false, true) => "artwork",
        (false, false) => "",
    };

    let mut steps: Vec<(Vec<&str>, &str)> = Vec::new();
    if matches!(target, BuildTarget::Html | BuildTarget::All) && !kind.is_empty() {
        steps.push((vec!["html", "--kind", kind], "HTML"));
    }
    if matches!(target, BuildTarget::Pdf | BuildTarget::All) && !kind.is_empty() {
        steps.push((vec!["pdf", "--kind", kind], "PDF"));
    }
    if matches!(target, BuildTarget::Slides | BuildTarget::All) {
        if has_presentation {
            steps.push((vec!["slides"], "Slides"));
        } else {
            failures.push(
                "Slides skipped: no presentation.md found (checked the project root, \
                 presennataion/, presentation/ and documentation/)."
                    .to_string(),
            );
        }
    }

    for (args, label) in steps {
        let mut full = vec![args[0]];
        full.extend_from_slice(&args[1..]);
        match run_engine_step(binary, &staging_root, project_id, &full, label) {
            Ok(text) => log.push_str(&text),
            Err(e) => failures.push(e.0),
        }
    }

    drop(staging);

    let exports_dir = project.join("exports");
    let mut exports: Vec<String> = fs::read_dir(&exports_dir)
        .map(|rd| {
            let mut files: Vec<String> = rd
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect();
            files.sort();
            files
        })
        .unwrap_or_default();

    if !failures.is_empty() {
        if !log.is_empty() {
            log.push('\n');
        }
        log.push_str(&failures.join("\n"));
        exports.clear();
    }

    Ok(BuildOutcome {
        success: failures.is_empty(),
        target: target.label().to_string(),
        output: log,
        exports,
        exports_dir: exports_dir.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths;

    #[test]
    fn engine_root_has_required_folders() {
        let root = paths::default_engine_root().expect("engine root discoverable");
        assert!(root.join("themes").is_dir());
        assert!(root.join("scripts").is_dir());
    }

    #[test]
    fn source_candidates_cover_archive_layout() {
        let tmp = std::env::temp_dir().join(format!("mi-src-cand-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("presennataion")).unwrap();
        fs::write(tmp.join("presennataion/presentation.md"), "# hi").unwrap();

        assert!(find_source(&tmp, "presentation.md").is_some());
        assert!(find_source(&tmp, "project-log.md").is_none());

        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn build_reports_missing_sources_clearly() {
        let tmp = std::env::temp_dir().join(format!("mi-build-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let engine_root = paths::default_engine_root().expect("engine root");
        let binary = paths::default_engine_binary().expect("engine binary");
        let err = build(&tmp, "X-1", &engine_root, &binary, BuildTarget::Html).unwrap_err();
        assert!(
            err.0.contains("No documentation sources found"),
            "{}",
            err.0
        );

        fs::remove_dir_all(&tmp).ok();
    }

    /// Full pipeline through the supplied engine binary and pandoc.
    /// Ignored by default because it shells out to external tools;
    /// run with `cargo test -- --ignored`.
    #[test]
    #[ignore = "runs pandoc via the supplied engine binary"]
    fn end_to_end_html_build() {
        let tmp = std::env::temp_dir().join(format!("mi-e2e-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(
            tmp.join("project-log.md"),
            "# Project Log\n\n## Status\n\nEnd-to-end test log.\n",
        )
        .unwrap();

        let engine_root = paths::default_engine_root().expect("engine root");
        let binary = paths::default_engine_binary().expect("engine binary");
        let outcome = build(
            &tmp,
            "MI-2026-E2E",
            &engine_root,
            &binary,
            BuildTarget::Html,
        )
        .expect("build command runs");

        assert!(outcome.success, "build failed:\n{}", outcome.output);
        assert!(
            outcome.exports.iter().any(|f| f.ends_with(".html")),
            "no HTML export produced: {:?}\n{}",
            outcome.exports,
            outcome.output
        );

        fs::remove_dir_all(&tmp).ok();
    }
}

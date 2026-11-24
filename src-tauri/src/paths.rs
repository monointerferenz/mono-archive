//! Locating the supplied DEV-RESOURCES folders from a working directory.
//!
//! In development the app runs inside the workspace, where `DEV-RESOURCES`
//! sits next to `AGENTS.md`. Packaged builds may start elsewhere, so every
//! lookup walks up from the current directory and the executable, and the
//! user can always override paths in settings.

use std::env;
use std::path::{Path, PathBuf};

fn walk_up(start: &Path, marker: &str) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        let candidate = dir.join(marker);
        if candidate.exists() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    None
}

fn search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(cwd) = env::current_dir() {
        roots.push(cwd);
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.to_path_buf());
        }
    }
    roots
}

/// Find a path relative to the workspace root (e.g. `DEV-RESOURCES/foo`).
pub fn find_workspace_path(relative: &str) -> Option<PathBuf> {
    for root in search_roots() {
        if let Some(found) = walk_up(&root, relative) {
            return Some(found);
        }
    }
    None
}

/// Default location of the built templating-engine binary.
pub fn default_engine_binary() -> Option<PathBuf> {
    find_workspace_path("DEV-RESOURCES/monointerferenz-templating-engine")
        .map(|root| root.join("target/release/monointerferenz-engine"))
        .filter(|p| p.exists())
        .or_else(|| {
            find_workspace_path("DEV-RESOURCES/monointerferenz-templating-engine")
                .map(|root| root.join("target/debug/monointerferenz-engine"))
                .filter(|p| p.exists())
        })
}

/// The engine's project root: the directory holding `themes/`, `scripts/`
/// and `mi-options.json`.
pub fn default_engine_root() -> Option<PathBuf> {
    find_workspace_path("DEV-RESOURCES/monointerferenz-templating-engine")
        .filter(|p| p.join("themes").is_dir() && p.join("scripts").is_dir())
}

/// Default archive template to clone for new projects.
pub fn default_template_path() -> Option<PathBuf> {
    find_workspace_path("DEV-RESOURCES/Project-Template/MI-2026-TE").filter(|p| p.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_resources_are_found_in_dev() {
        // Tests run with cwd = src-tauri, inside the workspace.
        assert!(
            default_engine_root().is_some(),
            "engine root should be discoverable"
        );
        assert!(
            default_template_path().is_some(),
            "template should be discoverable"
        );
    }
}

// Thin wrappers around the Tauri command layer.
import { invoke } from "@tauri-apps/api/core";

export const api = {
  getSettings: () => invoke("get_settings"),
  saveSettings: (settings) => invoke("save_settings", { settings }),
  listProjects: () => invoke("list_projects"),
  getProject: (dirName) => invoke("get_project", { dirName }),
  saveProject: (dirName, fields) => invoke("save_project", { dirName, fields }),
  createProject: (id, title) => invoke("create_project", { id, title }),
  buildDocuments: (dirName, target) => invoke("build_documents", { dirName, target }),
};

/// Tauri surfaces command errors as strings or serialized error values.
export function errorMessage(err) {
  if (typeof err === "string") return err;
  if (err && typeof err.message === "string") return err.message;
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}

/// Deep-clone the field object so edits never alias backend state.
export function cloneFields(fields) {
  return {
    ...fields,
    keywords: [...(fields.keywords ?? [])],
    software: [...(fields.software ?? [])],
    hardware: [...(fields.hardware ?? [])],
    relationsRevisedBy: [...(fields.relationsRevisedBy ?? [])],
    relationsRelated: [...(fields.relationsRelated ?? [])],
  };
}

/// Order-insensitive deep equality (key order differs between serialized
/// backend objects and locally rebuilt ones).
function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((k) => [k, canonical(value[k])]),
    );
  }
  return value;
}

export function sameFields(a, b) {
  return JSON.stringify(canonical(a)) === JSON.stringify(canonical(b));
}

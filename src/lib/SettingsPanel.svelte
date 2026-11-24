<script>
  import { onMount } from "svelte";
  import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
  import { api, errorMessage } from "./api.js";

  let { settings, onClose, onSaved, notify } = $props();

  let form = $state({
    archivePath: "",
    engineRoot: "",
    engineBinary: "",
    templatePath: "",
  });
  let saving = $state(false);
  let error = $state("");

  // Snapshot the current settings when the dialog opens.
  onMount(() => {
    form = {
      archivePath: settings?.archivePath ?? "",
      engineRoot: settings?.engineRoot ?? "",
      engineBinary: settings?.engineBinary ?? "",
      templatePath: settings?.templatePath ?? "",
    };
  });

  async function browse(target, isFile) {
    try {
      const picked = await openFileDialog({
        directory: !isFile,
        multiple: false,
        title:
          target === "archivePath"
            ? "Choose the archive folder"
            : target === "engineBinary"
              ? "Choose the engine binary"
              : target === "engineRoot"
                ? "Choose the engine project root"
                : "Choose the project template folder",
      });
      if (typeof picked === "string" && picked) {
        form[target] = picked;
      }
    } catch (e) {
      // A cancelled dialog is not an error worth a toast.
      const msg = errorMessage(e);
      if (!/cancel/i.test(msg)) notify("error", msg);
    }
  }

  async function submit() {
    saving = true;
    error = "";
    try {
      const saved = await api.saveSettings({ ...form });
      onSaved(saved);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      saving = false;
    }
  }

  const rows = [
    {
      key: "archivePath",
      label: "Archive folder",
      hint: "Directory holding the project folders (one manifest.toml per project).",
      isFile: false,
      required: true,
    },
    {
      key: "engineRoot",
      label: "Engine project root",
      hint: "Folder containing themes/, scripts/ and mi-options.json of the templating engine.",
      isFile: false,
    },
    {
      key: "engineBinary",
      label: "Engine binary",
      hint: "Path to monointerferenz-engine (built with cargo build --release).",
      isFile: true,
    },
    {
      key: "templatePath",
      label: "Project template",
      hint: "Template project cloned when creating a new archive project.",
      isFile: false,
    },
  ];
</script>

<div
  class="modal-backdrop"
  role="presentation"
  onclick={(e) => {
    if (e.target === e.currentTarget) onClose();
  }}
  onkeydown={(e) => {
    if (e.key === "Escape") onClose();
  }}
>
  <div class="modal modal--wide" role="dialog" aria-modal="true" aria-labelledby="settings-title">
    <p class="eyebrow">Configuration</p>
    <h2 id="settings-title" class="heading">Settings</h2>
    <p class="microcopy">
      Empty fields fall back to the detected defaults inside this workspace. Settings are stored
      in the app configuration directory.
    </p>

    <form
      onsubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      {#each rows as row (row.key)}
        <div class="field" class:field--invalid={row.required && !form[row.key].trim()}>
          <label class="label" for={"settings-" + row.key}>{row.label}</label>
          <div class="path-row">
            <input
              id={"settings-" + row.key}
              type="text"
              bind:value={form[row.key]}
              placeholder="auto-detected"
              autocomplete="off"
              spellcheck="false"
            />
            <button
              type="button"
              class="button"
              onclick={() => browse(row.key, row.isFile)}
            >
              Browse…
            </button>
          </div>
          <span class="microcopy">{row.hint}</span>
          {#if row.required && !form[row.key].trim()}
            <span class="field-error">Choose the archive folder before listing projects.</span>
          {/if}
        </div>
      {/each}

      {#if error}
        <div class="notice notice--error"><p class="build-log">{error}</p></div>
      {/if}

      <div class="button-row">
        <button type="submit" class="button button--accent" disabled={saving}>
          {saving ? "Saving…" : "Save settings"}
        </button>
        <button type="button" class="button button--quiet" onclick={onClose}>Cancel</button>
      </div>
    </form>
  </div>
</div>

<script>
  import { onMount, onDestroy } from "svelte";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { api, errorMessage, cloneFields, sameFields } from "./api.js";

  let { dirName, onBack, onRenamed, notify } = $props();

  const STATUS_VALUES = ["draft", "wip", "presentable", "archived"];
  const LEVEL_VALUES = ["none", "stub", "partial", "complete"];
  const DOC_VALUES = ["missing", "wip", "ready"];
  const VISIBILITY_VALUES = ["public", "unlisted", "private"];
  const VERSION_KIND_VALUES = ["original", "revision", "adaptation"];

  let detail = $state(null);
  let loadError = $state("");
  let f = $state(null);
  let original = $state(null);
  let raw = $state("");
  let warnings = $state([]);
  let fieldErrors = $state({});

  let saving = $state(false);
  let saveError = $state("");

  let building = $state(false);
  let buildTarget = $state("");
  let buildOutcome = $state(null);
  let buildError = $state("");

  let showRaw = $state(false);

  async function load() {
    loadError = "";
    try {
      detail = await api.getProject(dirName);
      f = toEditable(detail.fields);
      original = cloneFields(detail.fields);
      raw = detail.raw;
      warnings = detail.warnings ?? [];
      fieldErrors = { ...detail.fieldErrors };
      saveError = "";
      buildOutcome = null;
      buildError = "";
    } catch (e) {
      loadError = errorMessage(e);
    }
  }

  onMount(() => {
    load();
    window.addEventListener("keydown", onKeydown);
  });

  onDestroy(() => {
    window.removeEventListener("keydown", onKeydown);
  });

  function onKeydown(e) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      save();
    }
  }

  // ── Derived state ──────────────────────────────────────────────────
  // `f` keeps list fields as newline text; compare the wire form against
  // the last saved snapshot.
  const dirty = $derived(f && original ? !sameFields(fromEditable(f), original) : false);

  function idError(id) {
    if (!id.trim()) return "Archive ID must not be empty";
    if (id !== id.trim()) return "Archive ID must not start or end with whitespace";
    if (id === "." || id === "..") return "Archive ID must not be “.” or “..”";
    if (id.includes("/") || id.includes("\\")) return "Archive ID must not contain path separators";
    if (id.startsWith(".")) return "Archive ID must not start with a dot";
    if ([...id].some((c) => c.charCodeAt(0) < 32)) return "Archive ID must not contain control characters";
    return "";
  }

  function yearError(year) {
    const v = year.trim();
    if (v && !/^-?\d+$/.test(v)) return `“${year}” is not a whole number`;
    return "";
  }

  const liveErrors = $derived.by(() => {
    if (!f) return {};
    const errs = {};
    const id = idError(f.id);
    if (id) errs.id = id;
    const year = yearError(f.year);
    if (year) errs.year = year;
    return errs;
  });

  // Backend errors (from load or last save) win over the live mirror.
  const shownErrors = $derived({ ...liveErrors, ...fieldErrors });

  // ── Field helpers ──────────────────────────────────────────────────
  function listToText(list) {
    return (list ?? []).join("\n");
  }

  function textToList(text) {
    return text
      .split("\n")
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
  }

  // The editable state keeps list fields as newline text for simple editing.
  function toEditable(fields) {
    return {
      ...cloneFields(fields),
      keywordsText: listToText(fields.keywords),
      softwareText: listToText(fields.software),
      hardwareText: listToText(fields.hardware),
      revisedByText: listToText(fields.relationsRevisedBy),
      relatedText: listToText(fields.relationsRelated),
    };
  }

  function fromEditable(edit) {
    const { keywordsText, softwareText, hardwareText, revisedByText, relatedText, ...rest } = edit;
    return {
      ...rest,
      keywords: textToList(keywordsText),
      software: textToList(softwareText),
      hardware: textToList(hardwareText),
      relationsRevisedBy: textToList(revisedByText),
      relationsRelated: textToList(relatedText),
    };
  }

  // ── Saving ─────────────────────────────────────────────────────────
  async function save() {
    if (!f || saving) return;
    saving = true;
    saveError = "";
    try {
      const res = await api.saveProject(dirName, fromEditable(f));
      if (res.saved) {
        original = cloneFields(fromEditable(f));
        raw = res.raw;
        warnings = res.warnings ?? [];
        fieldErrors = {};
        const renameNote = res.renamed
          ? ` Project directory renamed to “${res.dirName}”.`
          : "";
        notify("success", `Manifest saved.${renameNote}`);
        if (res.renamed) {
          onRenamed(res.dirName);
        }
      } else {
        fieldErrors = res.fieldErrors ?? {};
        saveError = "Nothing was written — fix the highlighted fields and save again.";
      }
    } catch (e) {
      saveError = errorMessage(e);
    } finally {
      saving = false;
    }
  }

  function revert() {
    if (!original) return;
    f = toEditable(original);
    fieldErrors = {};
    saveError = "";
  }

  // ── Building documentation ─────────────────────────────────────────
  async function runBuild(target) {
    if (building) return;
    building = true;
    buildTarget = target;
    buildError = "";
    buildOutcome = null;
    try {
      buildOutcome = await api.buildDocuments(dirName, target);
      if (buildOutcome.success) {
        notify("success", `${buildOutcome.target} export finished.`);
      } else {
        buildError = "The engine reported problems — see the log.";
      }
    } catch (e) {
      buildError = errorMessage(e);
    } finally {
      building = false;
      buildTarget = "";
    }
  }

  async function openFolder(path) {
    try {
      await openPath(path);
    } catch (e) {
      notify("error", errorMessage(e));
    }
  }

  // ── Select options that preserve unknown manifest values ───────────
  function optionsWithCurrent(current, known) {
    const list = [...known];
    if (current && !known.includes(current)) list.unshift(current);
    return list;
  }
</script>

{#if loadError}
  <div class="section-head">
    <div>
      <p class="eyebrow">Project</p>
      <h1 class="heading">{dirName}</h1>
    </div>
    <button type="button" class="button" onclick={onBack}>← Back to overview</button>
  </div>
  <div class="notice notice--error">
    <p><strong>Could not open this project.</strong></p>
    <p class="build-log">{loadError}</p>
    <p class="button-row">
      <button type="button" class="button" onclick={load}>Try again</button>
      <button type="button" class="button button--quiet" onclick={onBack}>Back to overview</button>
    </p>
  </div>
{:else if !f}
  <div class="loading-row">Reading manifest…</div>
{:else}
  <div class="section-head project-head">
    <div>
      <p class="eyebrow">Project manifest</p>
      <h1 class="heading">{f.title || f.id || dirName}</h1>
      <p class="header-meta">{detail.path}</p>
    </div>
    <div class="project-head__state">
      <span class="chip {dirty ? 'chip--warn' : 'chip--ok'}">
        {dirty ? "unsaved changes" : "saved"}
      </span>
      <button type="button" class="button" onclick={onBack}>← Overview</button>
    </div>
  </div>

  <div class="project-layout">
    <div class="project-main">
      <form
        onsubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <fieldset>
          <legend class="label">Identity</legend>
          <div class="field-row">
            <div class="field" class:field--invalid={!!shownErrors.id}>
              <label class="label" for="f-id">Archive ID</label>
              <input id="f-id" bind:value={f.id} autocomplete="off" spellcheck="false" />
              {#if shownErrors.id}
                <span class="field-error">{shownErrors.id}</span>
              {:else}
                <span class="microcopy">
                  Directory name of the project inside the archive.
                </span>
              {/if}
            </div>
            <div class="field">
              <label class="label" for="f-year">Year</label>
              <input id="f-year" bind:value={f.year} inputmode="numeric" autocomplete="off" />
              {#if shownErrors.year}
                <span class="field-error">{shownErrors.year}</span>
              {:else}
                <span class="microcopy">Whole year, e.g. 2026.</span>
              {/if}
            </div>
          </div>
          <div class="field">
            <label class="label" for="f-title">Title</label>
            <input id="f-title" bind:value={f.title} autocomplete="off" />
          </div>
          <div class="field">
            <label class="label" for="f-type">Type</label>
            <input id="f-type" bind:value={f.projectType} autocomplete="off" />
            <span class="microcopy">Free text, e.g. Audio-Visual-Computation.</span>
          </div>
        </fieldset>

        <fieldset>
          <legend class="label">Status</legend>
          <div class="field-row">
            <div class="field">
              <label class="label" for="f-status">Status</label>
              <select id="f-status" bind:value={f.status}>
                {#each optionsWithCurrent(f.status, STATUS_VALUES) as v (v)}
                  <option value={v}>{v}</option>
                {/each}
              </select>
            </div>
            <div class="field">
              <label class="label" for="f-level">Documentation level</label>
              <select id="f-level" bind:value={f.documentationLevel}>
                {#each optionsWithCurrent(f.documentationLevel, LEVEL_VALUES) as v (v)}
                  <option value={v}>{v}</option>
                {/each}
              </select>
            </div>
          </div>
        </fieldset>

        <fieldset>
          <legend class="label">Tags &amp; tools</legend>
          <div class="field">
            <label class="label" for="f-keywords">Keywords</label>
            <textarea id="f-keywords" bind:value={f.keywordsText} rows="3"></textarea>
            <span class="microcopy">One keyword per line.</span>
          </div>
          <div class="field-row">
            <div class="field">
              <label class="label" for="f-software">Software</label>
              <textarea id="f-software" bind:value={f.softwareText} rows="4"></textarea>
            </div>
            <div class="field">
              <label class="label" for="f-hardware">Hardware</label>
              <textarea id="f-hardware" bind:value={f.hardwareText} rows="4"></textarea>
            </div>
          </div>
        </fieldset>

        <fieldset>
          <legend class="label">Version</legend>
          <div class="field-row">
            <div class="field">
              <label class="label" for="f-vkind">Kind</label>
              <select id="f-vkind" bind:value={f.versionKind}>
                {#each optionsWithCurrent(f.versionKind, VERSION_KIND_VALUES) as v (v)}
                  <option value={v}>{v}</option>
                {/each}
              </select>
            </div>
            <div class="field">
              <label class="label" for="f-vdate">Date</label>
              <input id="f-vdate" bind:value={f.versionDate} placeholder="YYYY or YYYY-MM" />
            </div>
          </div>
          <div class="field">
            <label class="label" for="f-vlabel">Label</label>
            <input id="f-vlabel" bind:value={f.versionLabel} />
          </div>
        </fieldset>

        <fieldset>
          <legend class="label">Description</legend>
          <div class="field">
            <label class="label" for="f-short">Short</label>
            <input id="f-short" bind:value={f.descriptionShort} />
          </div>
          <div class="field">
            <label class="label" for="f-long">Long</label>
            <textarea id="f-long" bind:value={f.descriptionLong} rows="6"></textarea>
          </div>
        </fieldset>

        <fieldset>
          <legend class="label">Documents</legend>
          <div class="field-row">
            <div class="field">
              <label class="label" for="f-artwork">Artwork sheet</label>
              <select id="f-artwork" bind:value={f.documentArtworkSheet}>
                {#each optionsWithCurrent(f.documentArtworkSheet, DOC_VALUES) as v (v)}
                  <option value={v}>{v}</option>
                {/each}
              </select>
            </div>
            <div class="field">
              <label class="label" for="f-presentation">Presentation</label>
              <select id="f-presentation" bind:value={f.documentPresentation}>
                {#each optionsWithCurrent(f.documentPresentation, DOC_VALUES) as v (v)}
                  <option value={v}>{v}</option>
                {/each}
              </select>
            </div>
          </div>
          <div class="field">
            <label class="label" for="f-technical">Technical description</label>
            <select id="f-technical" bind:value={f.documentTechnicalDescription}>
              {#each optionsWithCurrent(f.documentTechnicalDescription, DOC_VALUES) as v (v)}
                <option value={v}>{v}</option>
              {/each}
            </select>
          </div>
        </fieldset>

        <fieldset>
          <legend class="label">Website</legend>
          <label class="choice">
            <input type="checkbox" bind:checked={f.websitePublish} />
            <span>Publish this project on the website</span>
          </label>
          <div class="field-row">
            <div class="field">
              <label class="label" for="f-slug">Slug</label>
              <input id="f-slug" bind:value={f.websiteSlug} autocomplete="off" spellcheck="false" />
            </div>
            <div class="field">
              <label class="label" for="f-visibility">Visibility</label>
              <select id="f-visibility" bind:value={f.websiteVisibility}>
                {#each optionsWithCurrent(f.websiteVisibility, VISIBILITY_VALUES) as v (v)}
                  <option value={v}>{v}</option>
                {/each}
              </select>
            </div>
          </div>
        </fieldset>

        <fieldset>
          <legend class="label">Relations</legend>
          <div class="field-row">
            <div class="field">
              <label class="label" for="f-revised">Revised by</label>
              <textarea id="f-revised" bind:value={f.revisedByText} rows="3"></textarea>
              <span class="microcopy">One archive ID per line.</span>
            </div>
            <div class="field">
              <label class="label" for="f-related">Related</label>
              <textarea id="f-related" bind:value={f.relatedText} rows="3"></textarea>
              <span class="microcopy">One archive ID per line.</span>
            </div>
          </div>
        </fieldset>

        <fieldset>
          <legend class="label">Rights</legend>
          <div class="field-row">
            <div class="field">
              <label class="label" for="f-copyright">Copyright</label>
              <input id="f-copyright" bind:value={f.rightsCopyright} />
            </div>
            <div class="field">
              <label class="label" for="f-license">License</label>
              <input id="f-license" bind:value={f.rightsLicense} />
            </div>
          </div>
        </fieldset>

        <div class="action-bar">
          <button type="submit" class="button button--accent" disabled={saving || !dirty}>
            {saving ? "Saving…" : "Save manifest"}
          </button>
          <button
            type="button"
            class="button button--quiet"
            onclick={revert}
            disabled={saving || !dirty}
          >
            Revert
          </button>
          <span class="spacer"></span>
          <span class="microcopy">
            {#if saveError}
              <span class="field-error">{saveError}</span>
            {:else if dirty}
              unsaved changes — ⌘S to save
            {:else}
              manifest is up to date
            {/if}
          </span>
        </div>
      </form>
    </div>

    <aside class="context">
      {#if warnings.length || Object.keys(shownErrors).length}
        <div class="notice notice--warning">
          <p><strong>Check before saving</strong></p>
          <ul class="plain-list">
            {#each Object.entries(shownErrors) as [key, msg] (key)}
              <li><code>{key}</code>: {msg}</li>
            {/each}
            {#each warnings as w, i (i)}
              <li>{w}</li>
            {/each}
          </ul>
        </div>
      {/if}

      <div class="rail-section">
        <p class="eyebrow">Documentation build</p>
        <p class="microcopy">
          Runs the supplied monointerferenz templating engine against this project. Output lands
          in <code>exports/</code>.
        </p>
        <div class="button-row">
          <button
            type="button"
            class="button"
            onclick={() => runBuild("html")}
            disabled={building}
          >
            {building && buildTarget === "html" ? "Building…" : "HTML"}
          </button>
          <button
            type="button"
            class="button"
            onclick={() => runBuild("pdf")}
            disabled={building}
          >
            {building && buildTarget === "pdf" ? "Building…" : "PDF"}
          </button>
          <button
            type="button"
            class="button"
            onclick={() => runBuild("slides")}
            disabled={building}
          >
            {building && buildTarget === "slides" ? "Building…" : "Slides"}
          </button>
          <button
            type="button"
            class="button button--accent"
            onclick={() => runBuild("all")}
            disabled={building}
          >
            {building && buildTarget === "all" ? "Building…" : "All"}
          </button>
        </div>

        {#if buildError}
          <div class="notice notice--error">
            <p class="build-log">{buildError}</p>
          </div>
        {/if}

        {#if buildOutcome}
          <div class="notice" class:notice--error={!buildOutcome.success}>
            <p>
              <strong>{buildOutcome.target}</strong>
              {buildOutcome.success ? "finished." : "finished with problems."}
            </p>
            {#if buildOutcome.output}
              <pre class="build-log">{buildOutcome.output}</pre>
            {/if}
            {#if buildOutcome.exports?.length}
              <p class="label">Exports</p>
              <ul class="plain-list">
                {#each buildOutcome.exports as file (file)}
                  <li><code>{file}</code></li>
                {/each}
              </ul>
            {/if}
            <p class="button-row">
              <button
                type="button"
                class="button"
                onclick={() => openFolder(buildOutcome.exportsDir)}
              >
                Open exports folder
              </button>
            </p>
          </div>
        {/if}
      </div>

      <div class="rail-section">
        <p class="eyebrow">Project folder</p>
        <p class="microcopy">{detail.path}</p>
        <div class="button-row">
          <button type="button" class="button" onclick={() => openFolder(detail.path)}>
            Open in Finder
          </button>
        </div>
      </div>

      <div class="rail-section">
        <p class="eyebrow">Raw manifest</p>
        <button
          type="button"
          class="button button--quiet"
          onclick={() => (showRaw = !showRaw)}
        >
          {showRaw ? "Hide manifest.toml" : "Show manifest.toml"}
        </button>
        {#if showRaw}
          <pre class="build-log raw-manifest">{raw}</pre>
          <p class="microcopy">
            Unknown fields and comments are preserved when saving.
          </p>
        {/if}
      </div>
    </aside>
  </div>
{/if}

<script>
  import { api, errorMessage } from "./api.js";

  let { projects, loading, error, settings, onOpen, onRefresh, onOpenSettings, notify } =
    $props();

  let query = $state("");
  let createOpen = $state(false);
  let newId = $state("");
  let newTitle = $state("");
  let creating = $state(false);
  let createError = $state("");

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return projects;
    return projects.filter((p) =>
      [p.id, p.title, p.year, p.projectType, p.dirName]
        .join(" ")
        .toLowerCase()
        .includes(q),
    );
  });

  const noArchiveConfigured = $derived(!settings || !settings.archivePath);

  function statusChip(status) {
    switch (status) {
      case "presentable":
        return "chip chip--ok";
      case "wip":
        return "chip chip--warn";
      case "draft":
        return "chip chip--info";
      case "archived":
        return "chip chip--muted";
      default:
        return "chip chip--warn";
    }
  }

  function docChip(state) {
    switch (state) {
      case "ready":
        return "chip chip--ok";
      case "wip":
        return "chip chip--warn";
      default:
        return "chip chip--muted";
    }
  }

  function levelChip(level) {
    switch (level) {
      case "complete":
        return "chip chip--ok";
      case "partial":
        return "chip chip--warn";
      case "stub":
        return "chip chip--info";
      default:
        return "chip chip--muted";
    }
  }

  function idError(id) {
    const v = id;
    if (!v.trim()) return "Archive ID must not be empty";
    if (v !== v.trim()) return "Archive ID must not start or end with whitespace";
    if (v === "." || v === "..") return "Archive ID must not be “.” or “..”";
    if (v.includes("/") || v.includes("\\")) return "Archive ID must not contain path separators";
    if (v.startsWith(".")) return "Archive ID must not start with a dot";
    if ([...v].some((c) => c.charCodeAt(0) < 32)) return "Archive ID must not contain control characters";
    return "";
  }

  const createIdError = $derived(idError(newId));
  const createDisabled = $derived(creating || !!createIdError || !newTitle.trim());

  async function submitCreate() {
    if (createDisabled) return;
    creating = true;
    createError = "";
    try {
      const summary = await api.createProject(newId.trim(), newTitle.trim());
      notify("success", `Project ${summary.id} created.`);
      createOpen = false;
      newId = "";
      newTitle = "";
      await onRefresh();
      onOpen(summary.dirName);
    } catch (e) {
      createError = errorMessage(e);
    } finally {
      creating = false;
    }
  }

  function rowKeydown(e, dirName) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onOpen(dirName);
    }
  }
</script>

<div class="section-head overview-head">
  <div>
    <p class="eyebrow">Archive</p>
    <h1 class="heading">Projects</h1>
  </div>
  <div class="button-row overview-actions">
    <label class="field overview-search">
      <span class="label">Filter</span>
      <input type="search" placeholder="id, title, year, type…" bind:value={query} />
    </label>
    <button type="button" class="button" onclick={onRefresh} disabled={loading}>
      {loading ? "Loading…" : "Refresh"}
    </button>
    <button
      type="button"
      class="button button--accent"
      onclick={() => (createOpen = true)}
      disabled={noArchiveConfigured}
    >
      New project
    </button>
  </div>
</div>

{#if noArchiveConfigured}
  <div class="notice">
    <p><strong>No archive folder configured.</strong></p>
    <p>
      Choose the directory that holds your project folders — each project directory is named by
      its archive ID and contains a <code>manifest.toml</code>.
    </p>
    <p>
      <button type="button" class="button button--accent" onclick={onOpenSettings}>
        Choose archive folder
      </button>
    </p>
  </div>
{:else if error}
  <div class="notice notice--error">
    <p><strong>Could not read the archive.</strong></p>
    <p class="build-log">{error}</p>
    <p>
      <button type="button" class="button" onclick={onRefresh}>Try again</button>
      <button type="button" class="button button--quiet" onclick={onOpenSettings}>
        Check settings
      </button>
    </p>
  </div>
{:else if loading && projects.length === 0}
  <div class="loading-row">Reading manifests…</div>
{:else if projects.length === 0}
  <div class="notice">
    <p><strong>No projects found in this archive.</strong></p>
    <p>The folder is empty or contains no project directories yet.</p>
    <p>
      <button type="button" class="button button--accent" onclick={() => (createOpen = true)}>
        Create the first project
      </button>
    </p>
  </div>
{:else}
  <div class="table-wrap">
    <table>
      <thead>
        <tr>
          <th scope="col">ID</th>
          <th scope="col">Title</th>
          <th scope="col">Year</th>
          <th scope="col">Type</th>
          <th scope="col">Status</th>
          <th scope="col">Docs level</th>
          <th scope="col">Documents</th>
        </tr>
      </thead>
      <tbody>
        {#each filtered as p (p.dirName)}
          <tr
            class="row-link"
            tabindex="0"
            role="link"
            aria-label="Open {p.id || p.dirName}"
            onclick={() => onOpen(p.dirName)}
            onkeydown={(e) => rowKeydown(e, p.dirName)}
          >
            <td>
              <span class="row-id">{p.id || p.dirName}</span>
              {#if p.dirName !== p.id}
                <div class="microcopy">dir: {p.dirName}</div>
              {/if}
            </td>
            <td>
              {#if p.manifestError}
                <span class="chip chip--err" title={p.manifestError}>manifest problem</span>
                <div class="microcopy">{p.manifestError}</div>
              {:else if p.title}
                {p.title}
              {:else}
                <span class="microcopy">untitled</span>
              {/if}
            </td>
            <td>{p.year || "—"}</td>
            <td>{p.projectType || "—"}</td>
            <td>
              {#if p.manifestError}
                <span class="chip chip--err">unreadable</span>
              {:else}
                <span class={statusChip(p.status)}>{p.status || "unknown"}</span>
              {/if}
            </td>
            <td>
              {#if !p.manifestError}
                <span class={levelChip(p.documentationLevel)}>
                  {p.documentationLevel || "unknown"}
                </span>
              {:else}
                —
              {/if}
            </td>
            <td>
              {#if !p.manifestError}
                <span class="docs-chips">
                  <span class={docChip(p.documentArtworkSheet)} title="Artwork sheet">
                    A:{p.documentArtworkSheet || "?"}
                  </span>
                  <span class={docChip(p.documentPresentation)} title="Presentation">
                    P:{p.documentPresentation || "?"}
                  </span>
                  <span class={docChip(p.documentTechnicalDescription)} title="Technical description">
                    T:{p.documentTechnicalDescription || "?"}
                  </span>
                </span>
              {:else}
                —
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  {#if filtered.length === 0}
    <div class="loading-row">No project matches “{query}”.</div>
  {/if}
  {#if projects.some((p) => p.warnings?.length)}
    <div class="notice">
      <p><strong>Manifest notes</strong></p>
      <ul class="plain-list">
        {#each projects as p}
          {#each p.warnings ?? [] as w}
            <li><code>{p.id || p.dirName}</code>: {w}</li>
          {/each}
        {/each}
      </ul>
    </div>
  {/if}
{/if}

{#if createOpen}
  <div
    class="modal-backdrop"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) createOpen = false;
    }}
    onkeydown={(e) => {
      if (e.key === "Escape") createOpen = false;
    }}
  >
    <div class="modal" role="dialog" aria-modal="true" aria-labelledby="create-title">
      <p class="eyebrow">New project</p>
      <h2 id="create-title" class="heading">Create from template</h2>
      <p class="microcopy">
        The supplied project template is cloned and stamped with the new archive ID. The ID
        becomes the project directory name.
      </p>

      <form
        onsubmit={(e) => {
          e.preventDefault();
          submitCreate();
        }}
      >
        <div class="field" class:field--invalid={!!createIdError && newId.length > 0}>
          <label class="label" for="new-id">Archive ID</label>
          <input id="new-id" bind:value={newId} placeholder="MI-2026-XX" autocomplete="off" />
          {#if createIdError}
            <span class="field-error">{createIdError}</span>
          {/if}
        </div>
        <div class="field" class:field--invalid={!newTitle.trim() && newTitle.length > 0}>
          <label class="label" for="new-title">Title</label>
          <input id="new-title" bind:value={newTitle} placeholder="Project title" autocomplete="off" />
          {#if !newTitle.trim() && newTitle.length > 0}
            <span class="field-error">Title must not be empty</span>
          {/if}
        </div>

        {#if createError}
          <div class="notice notice--error"><p class="build-log">{createError}</p></div>
        {/if}

        <div class="button-row">
          <button type="submit" class="button button--accent" disabled={createDisabled}>
            {creating ? "Creating…" : "Create project"}
          </button>
          <button type="button" class="button button--quiet" onclick={() => (createOpen = false)}>
            Cancel
          </button>
        </div>
      </form>
    </div>
  </div>
{/if}

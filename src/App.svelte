<script>
  import { onMount } from "svelte";
  import Overview from "./lib/Overview.svelte";
  import ProjectView from "./lib/ProjectView.svelte";
  import SettingsPanel from "./lib/SettingsPanel.svelte";
  import { api, errorMessage } from "./lib/api.js";

  // ── Theme (data-theme on <body>, per the design system) ────────────
  let theme = $state(document.body.dataset.theme === "light" ? "light" : "dark");
  // let theme = "dark";

  $effect(() => {
    document.body.dataset.theme = theme;
    try {
      localStorage.setItem("mono-theme", theme);
    } catch {
      /* storage unavailable — theme still applies for this session */
    }
  });

  // ── App state ──────────────────────────────────────────────────────
  let settings = $state(null);
  let settingsOpen = $state(false);
  let route = $state({ name: "overview" });
  let projects = $state([]);
  let listLoading = $state(false);
  let listError = $state("");

  // ── Toasts ─────────────────────────────────────────────────────────
  let toasts = $state([]);
  let toastSeq = 0;

  function notify(kind, message) {
    const id = ++toastSeq;
    toasts = [...toasts, { id, kind, message }];
    const ttl = kind === "error" ? 9000 : 4000;
    setTimeout(() => {
      toasts = toasts.filter((t) => t.id !== id);
    }, ttl);
  }

  // ── Data loading ───────────────────────────────────────────────────
  async function loadSettings() {
    try {
      settings = await api.getSettings();
    } catch (e) {
      notify("error", `Could not load settings: ${errorMessage(e)}`);
    }
  }

  async function refreshProjects() {
    listLoading = true;
    listError = "";
    try {
      projects = await api.listProjects();
    } catch (e) {
      projects = [];
      listError = errorMessage(e);
    } finally {
      listLoading = false;
    }
  }

  onMount(async () => {
    try {
      const stored = localStorage.getItem("mono-theme");
      if (stored === "light" || stored === "dark") theme = stored;
    } catch {
      /* ignore */
    }
    await loadSettings();
    await refreshProjects();
  });

  // ── Navigation ─────────────────────────────────────────────────────
  function openProject(dirName) {
    route = { name: "project", dirName };
    window.scrollTo(0, 0);
  }

  function goOverview() {
    route = { name: "overview" };
    refreshProjects();
    window.scrollTo(0, 0);
  }

  function projectRenamed(newDirName) {
    route = { name: "project", dirName: newDirName };
    refreshProjects();
  }

  async function settingsSaved(newSettings) {
    settings = newSettings;
    settingsOpen = false;
    notify("success", "Settings saved.");
    await refreshProjects();
  }
</script>

<header class="site-header app-header">
  <div class="app-header__brand">
    <span class="wordmark">mono-archive</span>
    <span class="header-meta">project archive manager</span>
  </div>

  <nav class="site-nav app-header__nav" aria-label="Main">
    <button
      type="button"
      class="button button--quiet"
      class:is-current={route.name === "overview"}
      onclick={goOverview}
    >
      Overview
    </button>
    <button type="button" class="button button--quiet" onclick={() => (settingsOpen = true)}>
      Settings
    </button>
    <button
      type="button"
      class="button button--quiet"
      onclick={() => (theme = theme === "dark" ? "light" : "dark")}
      title="Toggle dark / light theme"
    >
      {theme === "dark" ? "Light" : "Dark"}
    </button>
  </nav>

  <div class="header-meta app-header__path" title={settings?.archivePath || "No archive folder set"}>
    {settings?.archivePath ? `archive: ${settings.archivePath}` : "no archive folder set"}
  </div>
</header>

<main class="app-main">
  {#if route.name === "overview"}
    <Overview
      {projects}
      loading={listLoading}
      error={listError}
      {settings}
      onOpen={openProject}
      onRefresh={refreshProjects}
      onOpenSettings={() => (settingsOpen = true)}
      {notify}
    />
  {:else}
    {#key route.dirName}
      <ProjectView
        dirName={route.dirName}
        onBack={goOverview}
        onRenamed={projectRenamed}
        {notify}
      />
    {/key}
  {/if}
</main>

{#if settingsOpen}
  <SettingsPanel
    {settings}
    onClose={() => (settingsOpen = false)}
    onSaved={settingsSaved}
    {notify}
  />
{/if}

<div class="toast-stack" role="status" aria-live="polite">
  {#each toasts as toast (toast.id)}
    <div class="toast toast--{toast.kind}">{toast.message}</div>
  {/each}
</div>

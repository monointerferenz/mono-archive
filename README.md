# mono-archive

macOS application for creating and managing monointerferenz project archives.
Built with Rust + Tauri 2 and Svelte 5 (Vite).

## What it does

- **Overview screen** — lists every project directory in the configured archive
  folder with its status, documentation level and document states, read live
  from each `manifest.toml`. Projects with a missing or broken manifest are
  still listed and flagged instead of being hidden.
- **Project screen** — edits the project manifest. Saves are format-preserving:
  unknown fields, comments and value styles in `manifest.toml` survive a save.
  The archive ID is the project directory name — changing it renames the
  directory on save. Invalid input is marked inline and reported per field; it
  never silently rewrites or drops data.
- **Documentation builds** — runs the supplied monointerferenz templating
  engine (`DEV-RESOURCES/monointerferenz-templating-engine`) as a subprocess
  for `html`, `pdf`, `slides` or `all` exports. Output lands in the project's
  `exports/` folder; the engine's own sources and binary are never modified.
- **Settings** — archive folder, engine project root, engine binary and project
  template path, with auto-detected defaults for this workspace. Stored in the
  app config directory (`settings.json`).

## Commands

```sh
npm install            # frontend dependencies
npm run dev            # Vite dev server (port 1420, strict)
npm run tauri dev      # run the app in development
npm run tauri build    # release build + .app bundle
npm run check          # svelte-check (0 errors / 0 warnings expected)
npm run build          # vite production build
```

Rust side (`src-tauri/`):

```sh
cargo test             # 15 unit tests (manifest round-trip incl. the real
                       # DEV-RESOURCES template, archive ops, settings, engine)
cargo test -- --ignored # + end-to-end HTML build through pandoc
cargo fmt --check
cargo clippy --all-targets
```

The engine must be built once for documentation exports:

```sh
cd DEV-RESOURCES/monointerferenz-templating-engine
cargo build --release   # → target/release/monointerferenz-engine
```

Documentation builds additionally need `pandoc` (html/pdf), `xelatex` (pdf)
and `marp` or `npx` (slides) on `PATH`; the app checks for them and reports
which one is missing.

## Layout

```
src/                     Svelte UI (App, Overview, ProjectView, SettingsPanel)
src/styles/              copy of the supplied monointerferenz.css design system
src/app.css              app-level styles on top of the design system
src-tauri/src/           Rust backend: archive, manifest, engine, settings, paths
DEV-RESOURCES/           supplied assets — project template, design system,
                         templating engine (untouched)
```

## Data contracts

- An archive is a directory of project directories; each project directory is
  named by its archive ID and contains a `manifest.toml`.
- `manifest.toml` is the source of truth for project state. The UI never keeps
  a competing status. Unknown or unhandled fields are preserved on save.
- The template's misspelled subfolder names (`presennataion`, `assests`) are
  part of the shipped structure and are preserved as-is; the engine integration
  looks in those locations when locating `presentation.md` and friends.

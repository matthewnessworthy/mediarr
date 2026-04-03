# CLAUDE.md

## Project overview

Mediarr is a cross-platform desktop and CLI application for renaming and organising movies, TV series, and anime files. It parses release-group filenames using the `hunch` crate (a Rust port of Python's guessit), applies user-defined naming templates, handles subtitle discovery and renaming, and supports both batch scanning and folder watching.

See `mediarr-prd.md` for the full product requirements document.

## Architecture

Cargo workspace with three crates:

- `crates/mediarr-core` — shared library containing ALL business logic. Both binaries depend on this. Never put business logic in the Tauri or CLI crates.
- `crates/mediarr-cli` — thin CLI binary using `clap`. Calls into mediarr-core.
- `crates/mediarr-tauri` — thin Tauri binary. `#[tauri::command]` functions are wrappers around mediarr-core.

Frontend lives in `frontend/` — Svelte + shadcn-svelte + TailwindCSS.

## Tech stack

| Layer | Technology |
|-------|-----------|
| Language | Rust (2021 edition) |
| Desktop shell | Tauri v2 |
| Frontend framework | Svelte (not React, not Vue) |
| UI components | shadcn-svelte |
| CSS | TailwindCSS |
| Filename parsing | `hunch` crate |
| CLI | `clap` (derive API) |
| Filesystem watching | `notify` crate |
| Database | SQLite via `rusqlite` (rename history, undo) |
| Config format | TOML via `serde` + `toml` crate |
| Config paths | `dirs` crate for platform-appropriate locations |
| Async runtime | `tokio` (Tauri v2 default) |

## Key design decisions

### Core library independence

`mediarr-core` must have zero knowledge of Tauri or any UI framework. It should be usable as a standalone Rust library. This means:

- No Tauri types in mediarr-core's public API
- No UI-specific error formatting
- All async operations use tokio primitives, not Tauri-specific ones
- Config, scanning, renaming, and watching all work headlessly

### Subtitles are dependents, not independent files

Subtitle files are never parsed in isolation. They are discovered relative to a parent video file and inherit the video's parsed metadata. The subtitle's output name is derived from the video's output name with language and type suffixes.

### Naming templates use `{variable}` syntax

Not printf-style, not regex, not shell-style. Templates look like: `{title}/Season {season:02}/{title} - S{season:02}E{episode:02}.{ext}`. The `:02` modifier means zero-padded to 2 digits. When a variable (like `{type}` in subtitle templates) resolves to an empty string, adjacent dots should collapse cleanly (no double dots in output).

### Config is TOML, not JSON

User-editable, comment-friendly. Stored at the platform config directory (`dirs::config_dir()/mediarr/config.toml`). Both CLI and GUI read/write the same file.

### History is SQLite, not flat files

Rename operations are recorded in SQLite (`dirs::data_dir()/mediarr/history.db`) for efficient querying, batch grouping, and undo support.

### Never delete source files

Renames are moves or copies. The application never deletes user files. "Ignore" for non-preferred subtitles means leaving them in place, not removing them.

## Code conventions

### Rust

- Use `thiserror` for error types in mediarr-core, `anyhow` in the binaries
- Prefer `&Path` / `PathBuf` over `String` for file paths
- Use `tracing` for structured logging (not `log` or `println!`)
- All public functions in mediarr-core should have doc comments
- Tests go in `#[cfg(test)] mod tests` within each module, integration tests in `tests/`
- Use `serde::{Serialize, Deserialize}` on all structs that cross the core→binary boundary

### Svelte/Frontend

- Use Svelte 5 runes syntax (`$state`, `$derived`, `$effect`) not legacy `$:` reactive declarations
- Components in `frontend/src/lib/components/`
- Tauri IPC calls via `@tauri-apps/api` `invoke()` function
- TypeScript throughout the frontend (`.svelte` files with `<script lang="ts">`)
- All Tauri command return types should have corresponding TypeScript interfaces

### File naming

- Rust: `snake_case.rs` for modules
- Svelte: `PascalCase.svelte` for components
- TypeScript: `camelCase.ts` for utilities, `PascalCase.ts` for types/interfaces

## Important crate notes

### hunch

- `hunch` is the filename parser. Use it as a library: `use hunch::Hunch;`
- It supports cross-file context — pass directory paths for better title/type detection
- It has 82% compatibility with guessit's test suite. The remaining 18% are edge cases (bonus content, sample clips, ambiguous specials). Don't try to fix hunch's parsing — work around limitations in mediarr-core and consider upstreaming fixes to hunch.
- hunch is MIT licensed

### notify

- Use `notify` v6+ for filesystem watching
- Always debounce events — files from torrent clients and browsers arrive progressively
- Default debounce: 5 seconds after last event for a given file

### Tauri v2

- Tauri commands are async by default
- Use `tauri::State<>` for shared state (config, watcher handles, database connection)
- Filesystem access from the frontend requires the `fs` plugin with appropriate permissions
- Native file/folder dialogs via `@tauri-apps/plugin-dialog`
- System tray is out of scope for v1 but the architecture should not preclude it

## Testing approach

- Unit tests for mediarr-core parsing, template rendering, subtitle matching, and language detection
- Use hunch's own test YAML files as reference for expected parsing behaviour
- Integration tests for scan → plan → rename → undo round-trip
- Frontend: no unit tests initially, rely on manual testing via `tauri dev`

## Build and run

```bash
# Development (GUI)
cd frontend && npm install
cargo tauri dev

# Development (CLI only)
cargo run -p mediarr-cli -- scan /path/to/folder

# Build release
cargo tauri build        # GUI
cargo build --release -p mediarr-cli  # CLI only

# Run tests
cargo test --workspace
```

## What NOT to do

- Don't put business logic in Tauri command handlers — they should be 1-5 line wrappers
- Don't use `unwrap()` in library code — propagate errors with `?`
- Don't hardcode paths — always use `dirs` crate or user config
- Don't parse filenames manually — always use `hunch`
- Don't store config in JSON — it's TOML
- Don't use React or any React-like patterns — this is a Svelte project
- Don't implement metadata API lookups (TMDb, TVDB) — that's v2 scope
- Don't implement NFO generation or artwork downloading — that's v2 scope
- Don't add a system tray — that's v2 scope

<!-- GSD:project-start source:PROJECT.md -->
## Project

**Mediarr**

Mediarr is a cross-platform desktop and CLI application for renaming and organising movies, TV series, and anime files. It parses release-group filenames using the `hunch` crate, identifies media metadata, applies user-defined naming templates, handles subtitle discovery and renaming, and supports both batch scanning and folder watching. It targets users frustrated with existing tools (FileBot, Sonarr) that are ugly, overcomplicated, or bundled with unwanted features like torrent/usenet management.

**Core Value:** Rename and organise media files correctly, beautifully, and effortlessly — a tool people actually enjoy using.

### Constraints

- **Tech stack**: Rust + Tauri v2 + Svelte + shadcn-svelte + TailwindCSS — decided and non-negotiable
- **Filename parsing**: Must use `hunch` crate — not a custom parser
- **Config format**: TOML — human-editable, comment-friendly
- **History storage**: SQLite via `rusqlite`
- **Architecture**: Cargo workspace with mediarr-core (all business logic), mediarr-cli, mediarr-tauri — core must have zero UI knowledge
- **Safety**: Never delete source files — renames are moves/copies only
- **Cross-platform**: macOS, Linux, Windows with platform-appropriate paths
- **Testing**: Robust test coverage from day one — unit tests for core modules, integration tests for the full pipeline. Run locally first, GitHub Actions CI later when repo is pushed
<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->
## Technology Stack

## Recommended Stack
### Core Framework (Rust Backend)
| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| Rust | 2021 edition | Language | Decided constraint. Cross-platform, safe, fast. | HIGH |
| Tauri | 2.10.x | Desktop shell | Decided constraint. Tauri v2 is mature (released Oct 2024, now at 2.10.3). Actively maintained with frequent releases. | HIGH |
| tokio | 1.50.x | Async runtime | Tauri v2's default async runtime. No reason to use anything else. | HIGH |
| hunch | 1.1.7 | Filename parsing | Decided constraint. Rust-native guessit port. Claims 99.8% accuracy. Published on crates.io, actively maintained. Supports batch parsing with directory context for better title/type detection. | HIGH |
| clap | 4.6.x | CLI framework | Decided constraint. Derive API (`#[derive(Parser)]`) for ergonomic CLI definition. Stable v4 line, very actively maintained. | HIGH |
| rusqlite | 0.39.0 | SQLite database | Decided constraint. Ergonomic SQLite wrapper. Use `bundled` feature to compile SQLite into binary -- avoids system dependency issues on all platforms. Bundles SQLite 3.51.3. | HIGH |
| serde | 1.0.228 | Serialization | De facto Rust serialization framework. Required for TOML config, JSON CLI output, and Tauri IPC. | HIGH |
| toml | 1.1.2 | TOML parsing | Decided constraint. Serde-compatible TOML parser/serializer. v1.1.2 supports TOML spec 1.1.0. | HIGH |
| notify | 8.2.0 | Filesystem watching | Cross-platform filesystem event library. **NOTE: CLAUDE.md says "v6+" but v6 never existed.** Versions went 5.x -> 7.x -> 8.x. Use v8.2.0 (latest stable). v9.0.0-rc.2 is in release candidate but not stable yet. | HIGH |
| notify-debouncer-full | 0.7.0 | Event debouncing | Companion to notify. Provides full-featured debouncing (file rename tracking, cache). Depends on notify ^8.2.0. Use this instead of rolling custom debounce logic. | HIGH |
| dirs | 6.0.0 | Platform paths | Decided constraint. Platform-appropriate config/data/cache directories. | HIGH |
| thiserror | 2.0.18 | Error types (library) | Standard for library error types. Use in mediarr-core. v2.0 is the current major line. | HIGH |
| anyhow | 1.0.102 | Error handling (binaries) | Standard for application-level error handling. Use in mediarr-cli and mediarr-tauri. | HIGH |
| tracing | 0.1.44 | Structured logging | Decided constraint. Async-aware structured logging. De facto Rust logging standard. | HIGH |
| tracing-subscriber | 0.3.23 | Log formatting | Required companion to tracing. Provides formatters, filters, and layer composition. | HIGH |
| isolang | 2.4.0 | ISO 639 language codes | Converts between ISO 639-1 (2-letter), 639-3 (3-letter) codes and English names. Statically embedded tables -- no runtime lookup needed. Enable `english_names` and `lowercase_names` features for subtitle language detection from folder/filename strings. | MEDIUM |
### Frontend (Svelte/Web)
| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| Svelte | 5.55.x | UI framework | Decided constraint. Svelte 5 with runes syntax ($state, $derived, $effect). Fully stable, actively maintained. | HIGH |
| SvelteKit | 2.55.x | App framework | Tauri officially recommends SvelteKit over plain Svelte. Provides routing, static adapter for Tauri. Use `@sveltejs/adapter-static` with SSR disabled. | HIGH |
| shadcn-svelte | 1.2.5 | UI components | Decided constraint. Copy-paste component library built on Bits UI. Fully supports Svelte 5 + Tailwind v4. Uses OKLCH colors and `tw-animate-css`. | HIGH |
| TailwindCSS | 4.2.x | CSS | Decided constraint. v4 uses CSS-first configuration (no `tailwind.config.js`). 5x faster full builds, 100x faster incremental. shadcn-svelte fully supports v4. | HIGH |
| @tauri-apps/api | 2.10.x | Tauri IPC | JavaScript bindings for Tauri commands. `invoke()` for calling Rust functions. Keep version synced with tauri crate. | HIGH |
| @tauri-apps/plugin-dialog | ~2.6.0 | Native dialogs | File/folder open/save dialogs. Keep version synced with Rust counterpart. | HIGH |
| @tauri-apps/plugin-fs | ~2.4.5 | Filesystem access | Frontend filesystem access with permission scoping. Keep version synced with Rust counterpart. | HIGH |
| bits-ui | latest | Headless primitives | Underlying headless components for shadcn-svelte. Install latest -- shadcn-svelte manages compatibility. | MEDIUM |
| tailwind-variants | latest | Style variants | Used by shadcn-svelte for component variant management. | HIGH |
| clsx + tailwind-merge | latest | Class utilities | Class name merging for Tailwind. Required by shadcn-svelte. | HIGH |
| tw-animate-css | latest | Animations | Replaces deprecated `tailwindcss-animate` for Tailwind v4. Required by shadcn-svelte. | HIGH |
| @lucide/svelte | latest | Icons | Icon library used by shadcn-svelte. Replaced `lucide-svelte` in Svelte 5 migration. | HIGH |
### Tauri Plugins (Rust Side)
| Plugin | Version | Purpose | Why | Confidence |
|--------|---------|---------|-----|------------|
| tauri-plugin-fs | 2.4.5 | Filesystem access | Required for frontend file operations with permission scoping. | HIGH |
| tauri-plugin-dialog | 2.6.0 | Native dialogs | Open folder/file dialogs for scan paths and output directories. | HIGH |
## Alternatives Considered
| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Language codes | `isolang` | `codes-iso-639` | `isolang` has better API surface (enum-based, direct conversions). `codes-iso-639` is more modular but adds complexity for our simple use case. |
| Language codes | `isolang` | `rust_iso639` | Less actively maintained. `isolang` has cleaner API. |
| Filesystem watching | `notify` 8.2.0 | `notify` 9.0.0-rc.2 | v9 is still in release candidate. Stick with stable v8 for production. Upgrade when v9 goes stable. |
| Debouncing | `notify-debouncer-full` | Custom debounce | The crate handles rename tracking, file caching, and cross-platform edge cases. Rolling custom debounce would duplicate effort and miss edge cases. |
| Database | `rusqlite` | `sqlx` | `rusqlite` is simpler for a single-file SQLite use case. `sqlx` is async but adds compile-time query checking complexity we don't need. We're not building a web server. |
| Database | `rusqlite` | `diesel` | Diesel's ORM is overkill for simple rename history. `rusqlite` gives direct SQL control with minimal abstraction. |
| TOML | `toml` | `toml_edit` | `toml_edit` preserves comments and formatting on round-trip. Consider for v2 if users report config file comments being lost. For v1, `toml` is simpler. |
| CSS | Tailwind v4 | Tailwind v3 | v4 is the current line. shadcn-svelte requires v4 in its latest release. No reason to use v3. |
| Frontend | SvelteKit | Plain Svelte | SvelteKit provides routing and static generation out of the box. Tauri docs recommend it. Plain Svelte requires manual routing (e.g., `svelte-spa-router`), which is unnecessary friction. |
## Version Pinning Strategy
# Cargo.toml (workspace)
## Critical Compatibility Notes
### Tauri Plugin Version Sync
### SvelteKit + Tauri Configuration
- Use `@sveltejs/adapter-static`
- Set `export const ssr = false` in root `+layout.ts`
- Set `export const prerender = true` in root `+layout.ts`
- Tauri load functions only run in the webview, so server-side rendering is not applicable
### shadcn-svelte + Tailwind v4
- No `tailwind.config.js` file needed
- Use `@theme inline` directive in CSS
- Colors are OKLCH, not HSL
- `tailwindcss-animate` is deprecated; use `tw-animate-css` instead
### Rust Edition
### notify Version Correction
### rusqlite `bundled` Feature
### hunch Crate Status
## Installation
# Initialize workspace and crates (from project root)
# Frontend (from project root)
# Tauri CLI (globally or per-project)
# Or: npm install -D @tauri-apps/cli
## Sources
- Tauri v2 releases: https://v2.tauri.app/release/ (verified 2.10.3 on 2026-03-04)
- Tauri docs (SvelteKit): https://v2.tauri.app/start/frontend/sveltekit/
- notify crate: https://docs.rs/crate/notify/latest (verified 8.2.0)
- notify-debouncer-full: https://docs.rs/crate/notify-debouncer-full/latest (verified 0.7.0)
- rusqlite: https://docs.rs/crate/rusqlite/latest (verified 0.39.0)
- clap: https://docs.rs/crate/clap/latest (verified 4.6.0)
- thiserror: https://docs.rs/crate/thiserror/latest (verified 2.0.18)
- anyhow: https://docs.rs/crate/anyhow/latest (verified 1.0.102)
- tracing: https://docs.rs/crate/tracing/latest (verified 0.1.44)
- tracing-subscriber: https://docs.rs/crate/tracing-subscriber/latest (verified 0.3.23)
- serde: https://docs.rs/crate/serde/latest (verified 1.0.228)
- toml: https://docs.rs/crate/toml/latest (verified 1.1.2)
- dirs: https://docs.rs/crate/dirs/latest (verified 6.0.0)
- tokio: https://docs.rs/crate/tokio/latest (verified 1.50.0)
- isolang: https://docs.rs/crate/isolang/latest (verified 2.4.0)
- hunch: https://docs.rs/crate/hunch/latest (verified 1.1.7)
- shadcn-svelte: https://www.shadcn-svelte.com/ (verified 1.2.5, Svelte 5 + Tailwind v4 support)
- shadcn-svelte Tailwind v4 migration: https://www.shadcn-svelte.com/docs/migration/tailwind-v4
- Svelte releases: https://github.com/sveltejs/svelte/releases (verified 5.55.x)
- SvelteKit: https://www.npmjs.com/package/@sveltejs/kit (verified 2.55.x)
- Tailwind CSS: https://github.com/tailwindlabs/tailwindcss/releases (verified 4.2.2)
- @tauri-apps/api: https://www.npmjs.com/package/@tauri-apps/api (verified 2.10.1)
- tauri-plugin-dialog: https://docs.rs/crate/tauri-plugin-dialog/latest (verified 2.6.0)
- tauri-plugin-fs: https://docs.rs/crate/tauri-plugin-fs/latest (verified 2.4.5)
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

Conventions not yet established. Will populate as patterns emerge during development.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

Architecture not yet mapped. Follow existing patterns found in the codebase.
<!-- GSD:architecture-end -->

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd:quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd:debug` for investigation and bug fixing
- `/gsd:execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->

<!-- GSD:profile-start -->
## Developer Profile

> Profile not yet configured. Run `/gsd:profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->

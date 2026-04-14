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
| Desktop shell | Tauri |
| Frontend framework | Svelte (not React, not Vue) |
| UI components | shadcn-svelte |
| CSS | TailwindCSS |
| Filename parsing | `hunch` crate |
| CLI | `clap` (derive API) |
| Filesystem watching | `notify` crate |
| Database | SQLite via `rusqlite` (rename history, undo) |
| Config format | TOML via `serde` + `toml` crate |
| Config paths | `dirs` crate for platform-appropriate locations |
| Async runtime | `tokio` (Tauri default) |

> **Versions:** For exact dependency versions, see `Cargo.toml` (Rust crates) and `frontend/package.json` (npm packages). This document describes technology choices, not version pins.

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

- Use `notify` for filesystem watching (see Cargo.toml for exact version)
- Always debounce events — files from torrent clients and browsers arrive progressively
- Default debounce: 5 seconds after last event for a given file

### Tauri

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

- **Tech stack**: Rust + Tauri + Svelte + shadcn-svelte + TailwindCSS — decided and non-negotiable
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

> **Version source of truth:** See `Cargo.toml` files for Rust crate versions and `frontend/package.json` for npm package versions. The tables below describe technology choices and rationale, not version pins.

### Core Framework (Rust Backend)
| Technology | Purpose | Why |
|------------|---------|-----|
| Rust (2021 edition) | Language | Decided constraint. Cross-platform, safe, fast. |
| Tauri | Desktop shell | Decided constraint. Actively maintained with frequent releases. |
| tokio | Async runtime | Tauri's default async runtime. |
| hunch | Filename parsing | Decided constraint. Rust-native guessit port. Supports batch parsing with directory context. |
| clap | CLI framework | Decided constraint. Derive API (`#[derive(Parser)]`) for ergonomic CLI definition. |
| rusqlite | SQLite database | Decided constraint. Ergonomic SQLite wrapper. Use `bundled` feature to compile SQLite into binary. |
| serde | Serialization | De facto Rust serialization framework. Required for TOML config, JSON CLI output, and Tauri IPC. |
| toml | TOML parsing | Decided constraint. Serde-compatible TOML parser/serializer. |
| notify | Filesystem watching | Cross-platform filesystem event library. Use stable release (not RC). |
| notify-debouncer-full | Event debouncing | Companion to notify. Provides full-featured debouncing (file rename tracking, cache). |
| dirs | Platform paths | Decided constraint. Platform-appropriate config/data/cache directories. |
| thiserror | Error types (library) | Standard for library error types. Use in mediarr-core. |
| anyhow | Error handling (binaries) | Standard for application-level error handling. Use in mediarr-cli and mediarr-tauri. |
| tracing | Structured logging | Decided constraint. Async-aware structured logging. De facto Rust logging standard. |
| tracing-subscriber | Log formatting | Required companion to tracing. Provides formatters, filters, and layer composition. |
| isolang | ISO 639 language codes | Converts between ISO 639-1/639-3 codes and English names. Enable `english_names` and `lowercase_names` features. |

### Frontend (Svelte/Web)
| Technology | Purpose | Why |
|------------|---------|-----|
| Svelte | UI framework | Decided constraint. Svelte 5 runes syntax ($state, $derived, $effect). |
| SvelteKit | App framework | Tauri officially recommends SvelteKit. Provides routing, static adapter. Use `@sveltejs/adapter-static` with SSR disabled. |
| shadcn-svelte | UI components | Decided constraint. Copy-paste component library built on Bits UI. Uses OKLCH colors and `tw-animate-css`. |
| TailwindCSS | CSS | Decided constraint. CSS-first configuration (no `tailwind.config.js`). |
| @tauri-apps/api | Tauri IPC | JavaScript bindings for Tauri commands. `invoke()` for calling Rust functions. Keep version synced with tauri crate. |
| @tauri-apps/plugin-dialog | Native dialogs | File/folder open/save dialogs. Keep version synced with Rust counterpart. |
| @tauri-apps/plugin-fs | Filesystem access | Frontend filesystem access with permission scoping. Keep version synced with Rust counterpart. |
| bits-ui | Headless primitives | Underlying headless components for shadcn-svelte. |
| tailwind-variants | Style variants | Used by shadcn-svelte for component variant management. |
| clsx + tailwind-merge | Class utilities | Class name merging for Tailwind. Required by shadcn-svelte. |
| tw-animate-css | Animations | Replaces deprecated `tailwindcss-animate`. Required by shadcn-svelte. |
| @lucide/svelte | Icons | Icon library used by shadcn-svelte. |

### Tauri Plugins (Rust Side)
| Plugin | Purpose | Why |
|--------|---------|-----|
| tauri-plugin-fs | Filesystem access | Required for frontend file operations with permission scoping. |
| tauri-plugin-dialog | Native dialogs | Open folder/file dialogs for scan paths and output directories. |

### Alternatives Considered
| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Language codes | `isolang` | `codes-iso-639` | `isolang` has better API surface (enum-based, direct conversions). |
| Language codes | `isolang` | `rust_iso639` | Less actively maintained. `isolang` has cleaner API. |
| Filesystem watching | `notify` (stable) | `notify` (RC) | Stick with stable releases for production. Upgrade when next major goes stable. |
| Debouncing | `notify-debouncer-full` | Custom debounce | The crate handles rename tracking, file caching, and cross-platform edge cases. |
| Database | `rusqlite` | `sqlx` | `rusqlite` is simpler for a single-file SQLite use case. |
| Database | `rusqlite` | `diesel` | Diesel's ORM is overkill for simple rename history. |
| TOML | `toml` | `toml_edit` | `toml_edit` preserves comments on round-trip. Consider for v2 if needed. |
| CSS | Tailwind v4 | Tailwind v3 | v4 is the current line. shadcn-svelte requires v4. |
| Frontend | SvelteKit | Plain Svelte | SvelteKit provides routing and static generation out of the box. |

### Critical Compatibility Notes
#### Tauri Plugin Version Sync
Keep Rust-side `tauri-plugin-*` versions in sync with JS-side `@tauri-apps/plugin-*` versions. Match major.minor between the two.

#### SvelteKit + Tauri Configuration
- Use `@sveltejs/adapter-static`
- Set `export const ssr = false` in root `+layout.ts`
- Set `export const prerender = true` in root `+layout.ts`
- Tauri load functions only run in the webview, so server-side rendering is not applicable

#### shadcn-svelte + Tailwind v4
- No `tailwind.config.js` file needed
- Use `@theme inline` directive in CSS
- Colors are OKLCH, not HSL
- `tailwindcss-animate` is deprecated; use `tw-animate-css` instead

#### rusqlite `bundled` Feature
Use the `bundled` feature to compile SQLite into the binary -- avoids system dependency issues on all platforms.

### Sources
- Tauri: https://v2.tauri.app/release/
- Tauri docs (SvelteKit): https://v2.tauri.app/start/frontend/sveltekit/
- shadcn-svelte: https://www.shadcn-svelte.com/
- shadcn-svelte Tailwind v4 migration: https://www.shadcn-svelte.com/docs/migration/tailwind-v4
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

> Generated from session analysis (73 messages, 21 projects) on 2026-03-22.
> This section is managed by `generate-claude-profile` -- do not edit manually.

### Communication & Responses
- **Terse-direct (HIGH):** Match the developer's style — deliver results, not conversation. No unnecessary clarifying questions. Act on the instruction given.
- **Code-only explanations (HIGH):** Skip walkthroughs, rationale paragraphs, and step-by-step narratives. A brief one-liner about approach is acceptable; lengthy explanations are not.
- **Fast decisions (HIGH):** Present a recommended option and proceed unless told otherwise. Do not present lengthy option comparisons.

### Working Style
- **Hypothesis-driven debugging (MEDIUM):** Focus on the specific area pointed to rather than broad investigation. This developer does their own diagnosis — provide targeted fixes.
- **Self-directed learning (MEDIUM):** Answer specific questions directly and concisely. No unsolicited tutorials or background context.
- **Backend-focused (HIGH):** Default to performance, reliability, cost, and operational concerns. When frontend decisions arise, ask for preferences.

### Boundaries
- **Scope-creep sensitive (LOW):** Stay tightly scoped to exactly what is asked. Do not add unrequested features, refactor adjacent code, or expand scope. Ask first.
- **Opinionated on tooling (LOW):** Respect existing tool and technology choices. Suggest the developer's established stack first.
<!-- GSD:profile-end -->

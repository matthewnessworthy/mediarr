<!-- generated-by: gsd-doc-writer -->
# Development

Guide for contributing to and developing Mediarr locally.

## Local Setup

1. Clone the repository:

   ```bash
   git clone git@github.com:matthewnessworthy/mediarr.git
   cd mediarr
   ```

2. Install frontend dependencies:

   ```bash
   cd frontend && npm install
   ```

3. Run the GUI in development mode (from the `frontend/` directory):

   ```bash
   npx tauri dev
   ```

   This starts the Vite dev server on `http://localhost:5173` and launches the Tauri window. The Tauri config (`crates/mediarr-tauri/tauri.conf.json`) runs `npm run dev` as its `beforeDevCommand` automatically.

4. Alternatively, run the CLI only (no frontend required):

   ```bash
   cargo run -p mediarr-cli -- scan /path/to/folder
   ```

### Environment Variables

Mediarr does not use `.env` files. All user configuration is stored in TOML at the platform config directory (see [CONFIGURATION.md](CONFIGURATION.md)). Logging verbosity can be controlled via the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug cargo run -p mediarr-cli -- scan /path/to/folder
```

## Build Commands

### Rust (workspace root)

| Command | Description |
|---------|-------------|
| `cargo build --workspace` | Build all crates (debug) |
| `cargo build --release -p mediarr-cli` | Release build of the CLI binary |
| `cargo test --workspace` | Run all unit and integration tests |
| `cargo fmt --all --check` | Check Rust formatting (enforced in CI) |
| `cargo clippy --workspace -- -D warnings` | Run Clippy lints with warnings-as-errors (enforced in CI) |
| `cargo fmt --all` | Auto-format all Rust code |
| `cargo doc --workspace --no-deps` | Generate Rust API docs |

### Frontend (`frontend/` directory)

| Command | Description |
|---------|-------------|
| `npm run dev` | Start Vite dev server on port 5173 |
| `npm run build` | Production build (static output via `@sveltejs/adapter-static`) |
| `npm run preview` | Preview production build locally |
| `npm run check` | Run `svelte-kit sync` and `svelte-check` for type checking |
| `npm run test` | Run Vitest unit tests |
| `npm run test:e2e` | Run Playwright end-to-end tests |

### Tauri (from `frontend/` directory)

| Command | Description |
|---------|-------------|
| `npx tauri dev` | Launch dev mode (Vite + Tauri window) |
| `npx tauri build` | Build release desktop app |

## Code Style

Mediarr does not use ESLint, Prettier, or Biome. Formatting and linting are handled by language-native tooling:

- **Rust:** `rustfmt` for formatting, `clippy` for linting. Both are enforced in CI via `cargo fmt --all --check` and `cargo clippy --workspace -- -D warnings`. Run `cargo fmt --all` before committing Rust changes.
- **TypeScript/Svelte:** `svelte-check` for type checking (`npm run check` in `frontend/`). No dedicated formatter is configured for the frontend.
- **Tailwind CSS v4:** CSS-first configuration (no `tailwind.config.js`). Styles use OKLCH color values and `tw-animate-css`.

### File Naming Conventions

- Rust modules: `snake_case.rs`
- Svelte components: `PascalCase.svelte`
- TypeScript utilities: `camelCase.ts`
- TypeScript types/interfaces: `PascalCase.ts`

### Rust Conventions

- `thiserror` for error types in `mediarr-core`; `anyhow` in binary crates
- `&Path` / `PathBuf` for file paths, not `String`
- `tracing` for structured logging (not `log` or `println!`)
- Doc comments on all public functions in `mediarr-core`
- Unit tests in `#[cfg(test)] mod tests` blocks; integration tests in `tests/`

### Svelte Conventions

- Svelte 5 runes syntax (`$state`, `$derived`, `$effect`) -- not legacy `$:` reactive declarations
- TypeScript in all `.svelte` files (`<script lang="ts">`)
- Tauri IPC via `@tauri-apps/api` `invoke()` function

## Project Structure

```
mediarr/
  crates/
    mediarr-core/        # Shared library -- ALL business logic lives here
      src/
        config.rs        # TOML config loading/saving
        error.rs         # Error types (thiserror)
        fs_util.rs       # Filesystem utilities
        history.rs       # SQLite rename history and undo
        parser.rs        # Filename parsing via hunch
        renamer.rs       # File rename/move execution
        scanner.rs       # Directory scanning
        subtitle.rs      # Subtitle discovery and matching
        template.rs      # Naming template rendering
        types.rs         # Shared types (serde-serializable)
        watcher.rs       # Filesystem watching (notify)
      tests/
        integration_test.rs
    mediarr-cli/         # Thin CLI binary (clap)
      src/
        main.rs
        output.rs        # CLI output formatting
        commands/        # One file per CLI subcommand
          config.rs
          history.rs
          rename.rs
          review.rs
          scan.rs
          undo.rs
          watch.rs
      tests/
        cli_integration.rs
    mediarr-tauri/       # Thin Tauri binary -- wrappers around mediarr-core
      src/
        main.rs
        lib.rs
        state.rs         # Tauri shared state (config, DB, watcher handles)
        error.rs         # Tauri error conversions
        commands/        # One file per Tauri command group
          config.rs
          history.rs
          rename.rs
          scan.rs
          watcher.rs
  frontend/              # Svelte + SvelteKit + shadcn-svelte
    src/
      lib/
        components/
          ui/            # shadcn-svelte primitives (badge, button, input, etc.)
          scan/          # Scan view components
          history/       # History view components
          settings/      # Settings view components
          watcher/       # Watcher view components
        state/           # Svelte 5 rune-based state stores
          config.svelte.ts
          history.svelte.ts
          scan.svelte.ts
          theme.svelte.ts
          watcher.svelte.ts
        utils.ts         # Shared utilities
        types/           # TypeScript type definitions
      routes/
        +layout.svelte
        +layout.ts       # SSR disabled, prerender enabled
        +page.svelte     # Root page (scan view)
        scan/
        history/
        settings/
        watcher/
      test/
        setup.ts         # Vitest global setup
        fixtures.ts      # Test fixture data
        mocks.ts         # Tauri IPC mocks
    e2e/                 # Playwright end-to-end tests
      helpers/
      navigation.spec.ts
      scan.spec.ts
      history.spec.ts
      settings.spec.ts
      watcher.spec.ts
```

### Architecture Boundaries

- **`mediarr-core`** has zero knowledge of Tauri or any UI framework. It is a standalone Rust library.
- **`mediarr-cli`** and **`mediarr-tauri`** are thin wrappers (1-5 lines per command handler) that call into `mediarr-core`.
- Business logic must never be placed in the CLI or Tauri crates.

## Branch Conventions

Commits follow the [Conventional Commits](https://www.conventionalcommits.org/) style with type prefixes:

- `feat:` -- new features
- `fix:` -- bug fixes
- `test:` -- test additions or changes
- `refactor:` -- code restructuring without behavior changes
- `chore:` -- maintenance, config, tooling changes

Scoped prefixes (e.g., `feat(quick-260407-pol):`) are used for task-tracking context. The default branch is `main`.

## PR Process

No `CONTRIBUTING.md` or PR template is configured yet. When submitting changes:

- Ensure `cargo fmt --all --check` passes
- Ensure `cargo clippy --workspace -- -D warnings` passes
- Ensure `cargo test --workspace` passes
- Ensure `npm run check` passes in `frontend/`
- Ensure `npm run test` passes in `frontend/`

CI runs these checks automatically on push to `main` and on pull requests (see `.github/workflows/ci.yml`). The CI matrix tests Rust on macOS, Ubuntu, and Windows. Frontend tests (Vitest + Playwright) run on Ubuntu.

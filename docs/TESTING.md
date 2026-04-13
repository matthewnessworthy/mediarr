<!-- generated-by: gsd-doc-writer -->
# Testing

Mediarr uses a multi-layer testing strategy across its Rust backend and Svelte frontend. The Rust workspace relies on the built-in `cargo test` runner with inline unit tests and integration test files. The frontend uses Vitest for unit tests and Playwright for end-to-end browser tests.

## Test framework and setup

### Rust (cargo test)

All three workspace crates use the standard Rust test harness via `cargo test`. No additional test framework is required beyond the dev-dependencies declared in each crate's `Cargo.toml`:

| Crate | Dev Dependencies |
|-------|-----------------|
| `mediarr-core` | `tempfile` |
| `mediarr-cli` | `assert_cmd`, `assert_fs`, `predicates`, `tempfile`, `serde_json` |
| `mediarr-tauri` | `tempfile` |

Setup: no global setup is needed. Tests create temporary directories via `tempfile::TempDir` and are fully self-contained.

### Frontend (Vitest)

Unit tests use **Vitest** with a jsdom environment. Configuration is in `frontend/vitest.config.ts`:

- Environment: `jsdom`
- Test file patterns: `src/**/*.test.ts`, `src/**/*.svelte.test.ts`
- Setup file: `src/test/setup.ts` (provides WebCrypto polyfill for jsdom, clears Tauri IPC mocks after each test)

### Frontend (Playwright)

End-to-end tests use **Playwright** targeting Chromium. Configuration is in `frontend/playwright.config.ts`:

- Test directory: `frontend/e2e/`
- Base URL: `http://localhost:5173`
- Dev server: started automatically via `npm run dev` with `VITE_PLAYWRIGHT=true`
- Retries: 2 in CI, 0 locally
- Workers: 1 in CI, auto locally

## Running tests

### Rust tests

```bash
# Run all workspace tests (core + cli + tauri)
cargo test --workspace

# Run tests for a specific crate
cargo test -p mediarr-core
cargo test -p mediarr-cli
cargo test -p mediarr-tauri

# Run a single test by name
cargo test -p mediarr-core -- template::tests::render_movie_basic

# Run tests matching a pattern
cargo test -p mediarr-core -- parser::tests

# Show test output (println, tracing)
cargo test --workspace -- --nocapture
```

### Frontend unit tests (Vitest)

```bash
cd frontend

# Run all unit tests
npm test               # alias for: vitest run

# Run in watch mode
npx vitest

# Run a specific test file
npx vitest run src/lib/state/scan.svelte.test.ts
```

### Frontend E2E tests (Playwright)

```bash
cd frontend

# Run all E2E tests (starts dev server automatically)
npm run test:e2e       # alias for: playwright test

# Run a specific test file
npx playwright test e2e/navigation.spec.ts

# Run with visible browser
npx playwright test --headed

# View HTML report after a run
npx playwright show-report
```

## Writing new tests

### Rust test conventions

**Unit tests** are co-located with source code using inline `#[cfg(test)] mod tests` blocks. Every module in `mediarr-core` has its own test block:

- `crates/mediarr-core/src/config.rs` -- config loading, saving, TOML round-trips
- `crates/mediarr-core/src/parser.rs` -- filename parsing, media type detection
- `crates/mediarr-core/src/template.rs` -- template rendering, variable substitution
- `crates/mediarr-core/src/subtitle.rs` -- subtitle discovery, language detection
- `crates/mediarr-core/src/scanner.rs` -- folder scanning, conflict detection
- `crates/mediarr-core/src/renamer.rs` -- dry run, execute, file operations
- `crates/mediarr-core/src/history.rs` -- SQLite operations, batch management, undo
- `crates/mediarr-core/src/watcher.rs` -- filesystem watching, auto/review modes
- `crates/mediarr-core/src/types.rs` -- type serialization, display traits, filtering
- `crates/mediarr-core/src/fs_util.rs` -- path utilities, safe move operations

**Integration tests** live in `tests/` directories within each crate:

| File | Purpose |
|------|---------|
| `crates/mediarr-core/tests/integration_test.rs` | Full scan -> plan -> rename -> history -> undo round-trip |
| `crates/mediarr-cli/tests/cli_integration.rs` | CLI binary end-to-end tests via `assert_cmd` |
| `crates/mediarr-tauri/tests/command_tests.rs` | Tauri command handler logic tests (exercises core APIs the same way commands do) |

**Patterns:**
- Use `tempfile::TempDir` for all filesystem operations -- never write to real paths
- Create fake media files with `fs::write(&path, b"fake content")` -- file content does not matter for parsing
- Helper functions (e.g., `config_with_output()`, `test_media_info()`) are defined at the top of each test file

### Frontend test conventions

**Unit tests** follow the `*.test.ts` or `*.svelte.test.ts` naming pattern:

| File | Tests |
|------|-------|
| `src/lib/utils.test.ts` | Utility function tests (Tailwind class merging) |
| `src/lib/state/scan.svelte.test.ts` | Scan state management (filtering, selection, conflicts) |
| `src/lib/state/config.svelte.test.ts` | Config state management |
| `src/lib/state/history.svelte.test.ts` | History state management (expand/collapse) |
| `src/lib/state/watcher.svelte.test.ts` | Watcher state management |
| `src/lib/state/theme.svelte.test.ts` | Theme state (dark/light toggle, localStorage persistence) |

**Test helpers** in `frontend/src/test/`:

- `fixtures.ts` -- Factory functions (`mockScanResult()`, `mockMediaInfo()`, `mockConfig()`, `mockBatchSummary()`, `mockWatcherConfig()`, `mockWatcherEvent()`, etc.) for creating test data with partial overrides
- `mocks.ts` -- IPC mock handler factories (`mockScanHandlers()`, `mockConfigHandlers()`, `mockHistoryHandlers()`, `mockWatcherHandlers()`, `allMockHandlers()`) for Tauri command mocking in unit tests
- `setup.ts` -- Global test setup: WebCrypto polyfill for jsdom, Tauri mock cleanup after each test

**E2E tests** follow the `*.spec.ts` naming pattern in `frontend/e2e/`:

| File | Tests |
|------|-------|
| `e2e/navigation.spec.ts` | Sidebar navigation, active state highlighting, theme toggle |
| `e2e/scan.spec.ts` | Scan view empty state, folder selection |
| `e2e/settings.spec.ts` | Settings view |
| `e2e/history.spec.ts` | History view |
| `e2e/watcher.spec.ts` | Watcher view |

E2E tests use a shared helper at `e2e/helpers/mock-setup.ts` that provides `gotoWithMocks(page, path, overrides?)` -- this injects Tauri IPC mocks via `addInitScript` before page load, ensuring commands called during `onMount` are intercepted.

## Coverage requirements

No coverage threshold is configured for either Rust or frontend tests. Coverage is not enforced in CI.

To generate Rust coverage locally (requires `cargo-llvm-cov` or `cargo-tarpaulin`):

```bash
# With cargo-tarpaulin
cargo tarpaulin --workspace
```

To generate frontend coverage locally:

```bash
cd frontend
npx vitest run --coverage
```

## CI integration

Tests run automatically via GitHub Actions on every push to `main` and every pull request targeting `main`. The workflow is defined in `.github/workflows/ci.yml`.

### Rust job (`rust`)

- **Runs on:** macOS, Ubuntu, and Windows (matrix strategy, `fail-fast: false`)
- **Steps:**
  1. Install Linux dependencies (webkit2gtk, appindicator, etc.) on Ubuntu
  2. Install stable Rust toolchain with `clippy` and `rustfmt` components
  3. `cargo fmt --all --check` -- formatting check
  4. `cargo clippy --workspace -- -D warnings` -- lint check (warnings are errors)
  5. `cargo test --workspace` -- full test suite

### Frontend job (`frontend`)

- **Runs on:** Ubuntu
- **Steps:**
  1. Set up Node.js (LTS) with npm cache
  2. `npm ci` -- install dependencies
  3. `npx vitest run` -- unit tests
  4. Cache Playwright browsers (keyed on `package-lock.json` hash)
  5. Install Playwright Chromium (with deps)
  6. `npx playwright test` -- E2E tests

## Test counts

As of last count, the workspace contains approximately:

- **369 Rust tests** across all three crates (335 in `mediarr-core`, 10 in `mediarr-cli`, 20 in `mediarr-tauri`, 4 integration tests in `mediarr-core`)
- **6 frontend unit test files** covering state management and utilities
- **5 frontend E2E test files** covering navigation, scan, settings, history, and watcher views

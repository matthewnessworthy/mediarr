<!-- generated-by: gsd-doc-writer -->
# Contributing to Mediarr

Thank you for your interest in contributing to Mediarr. This document covers the essentials for getting started and submitting changes.

## Development Setup

See [docs/GETTING-STARTED.md](docs/GETTING-STARTED.md) for prerequisites and first-run instructions, and [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for local development setup, build commands, and project structure.

## Coding Standards

- **Rust formatting:** `cargo fmt --all` before committing. CI enforces `cargo fmt --all --check` and will fail on unformatted code.
- **Rust linting:** `cargo clippy --workspace -- -D warnings`. All warnings are treated as errors in CI.
- **Frontend type checking:** `npm run check` in the `frontend/` directory runs `svelte-kit sync` and `svelte-check`.
- **Tests:** `cargo test --workspace` for Rust, `npm test` for frontend unit tests, and `npm run test:e2e` for Playwright E2E tests. All are enforced in CI.

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for the complete list of build commands, file naming conventions, and code style details.

## PR Guidelines

- **Commit messages:** Follow [Conventional Commits](https://www.conventionalcommits.org/) format with type prefixes: `feat:`, `fix:`, `test:`, `refactor:`, `chore:`.
- **All CI checks must pass:** formatting (`cargo fmt`), linting (`cargo clippy`), Rust tests, frontend type checks, Vitest, and Playwright. CI runs on macOS, Ubuntu, and Windows for Rust.
- **Business logic belongs in `mediarr-core`:** The CLI and Tauri crates are thin wrappers only. Do not add logic to command handlers.
- **No `unwrap()` in library code:** Propagate errors with `?` in `mediarr-core`. Use `thiserror` for error types in core, `anyhow` in binaries.
- **Test your changes:** Add or update tests in the relevant crate. Use `tempfile::TempDir` for filesystem operations -- never write to real paths.
- **Keep PRs focused:** One feature or fix per pull request.

## Issue Reporting

Report bugs and request features via [GitHub Issues](https://github.com/matthewnessworthy/mediarr/issues). When filing a bug report, include:

- Steps to reproduce the issue
- Expected behavior vs actual behavior
- Operating system and version
- Rust and Node.js versions (`rustc --version`, `node --version`)
- Relevant log output (set `RUST_LOG=debug` for verbose logging)

For feature requests, describe the use case and how it fits within Mediarr's scope as a media file renaming and organisation tool.

## License

By contributing, you agree that your contributions will be licensed under the [GPL-3.0-only](LICENSE) license.

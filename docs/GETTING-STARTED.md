<!-- generated-by: gsd-doc-writer -->
# Getting Started

This guide walks you through setting up Mediarr for the first time, from installing prerequisites to running your first media file scan.

## Prerequisites

Mediarr is a Rust + Tauri + Svelte application. You need the following tools installed before building.

### Rust

- **Rust stable toolchain** (edition 2021) -- install via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Verify with:

```bash
rustc --version   # Rust 1.70+ recommended
cargo --version
```

### Node.js

- **Node.js LTS** (the CI uses `lts/*`) and **npm**:

```bash
node --version   # v20+ recommended
npm --version
```

### Tauri System Dependencies

Tauri requires platform-specific system libraries for the webview.

**macOS:** No extra dependencies -- Xcode Command Line Tools are sufficient.

```bash
xcode-select --install
```

**Linux (Debian/Ubuntu):**

```bash
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

**Windows:** Install [Microsoft Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) and [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (usually pre-installed on Windows 10+).

<!-- VERIFY: Windows WebView2 may already be included in recent Windows versions -->

For the full list of Tauri prerequisites, see the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/).

## Installation Steps

1. Clone the repository:

```bash
git clone git@github.com:matthewnessworthy/mediarr.git
cd mediarr
```

2. Install frontend dependencies:

```bash
cd frontend && npm install && cd ..
```

3. Build the Rust workspace (this also compiles the bundled SQLite):

```bash
cargo build --workspace
```

## First Run

### GUI (Tauri Desktop App)

Run the Tauri development server, which launches the desktop window with hot-reloading for the Svelte frontend:

```bash
cd frontend && npm install
cargo tauri dev
```

This starts the Vite dev server on `http://localhost:5173` and opens the Tauri window. The app has four main views: scan, history, watcher, and settings.

### CLI Only

If you only need the command-line tool, run a scan against a folder of media files:

```bash
cargo run -p mediarr-cli -- scan /path/to/your/media
```

The CLI binary is named `mediarr` and supports these subcommands:

| Command   | Description                                        |
|-----------|----------------------------------------------------|
| `scan`    | Scan a folder for media files and show rename proposals |
| `rename`  | Rename media files according to naming templates   |
| `history` | Show rename history                                |
| `undo`    | Undo a previous rename batch                       |
| `watch`   | Watch a folder for new media files                 |
| `config`  | View or modify configuration                       |
| `review`  | Review queued rename proposals from watch mode     |

Example dry-run scan:

```bash
cargo run -p mediarr-cli -- scan /path/to/media --dry-run
```

## Common Setup Issues

### Missing Tauri system dependencies on Linux

**Symptom:** Build fails with errors about missing `webkit2gtk` or `libappindicator`.

**Fix:** Install the required system libraries listed in the prerequisites section above.

### `cargo tauri dev` command not found

**Symptom:** `error: no such command: tauri`

**Fix:** The Tauri CLI is an npm devDependency in `frontend/package.json`. Make sure you run `cargo tauri dev` from the project root after running `npm install` in the `frontend/` directory. Alternatively, use `npx tauri dev` from the `frontend/` directory.

### SQLite build errors

**Symptom:** Compilation errors related to SQLite or `libsqlite3-sys`.

**Fix:** The `rusqlite` dependency uses the `bundled` feature, which compiles SQLite from source. Ensure you have a C compiler available (included with Xcode CLI tools on macOS, `build-essential` on Ubuntu, or Visual Studio Build Tools on Windows).

### Frontend not loading in Tauri window

**Symptom:** The Tauri window opens but shows a blank page or connection error.

**Fix:** The Tauri config expects the Vite dev server at `http://localhost:5173`. Ensure nothing else is using that port, or check that `npm run dev` started successfully in the `frontend/` directory.

## Next Steps

- **[ARCHITECTURE.md](ARCHITECTURE.md)** -- Understand the workspace structure, crate boundaries, and data flow.
- **[CONFIGURATION.md](CONFIGURATION.md)** -- Learn about the TOML config file, naming templates, and all available settings.
- **[README.md](../README.md)** -- Project overview and quick reference.

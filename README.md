<!-- generated-by: gsd-doc-writer -->
# Mediarr

A cross-platform desktop and CLI application for renaming and organising movies, TV series, and anime files -- built for users frustrated with ugly, overcomplicated tools like FileBot and Sonarr.

## Installation

### Prerequisites

- **Rust** (2021 edition) with `cargo`
- **Node.js** (for frontend build)
- **npm** (ships with Node.js)

### Build from source

```bash
git clone git@github.com:matthewnessworthy/mediarr.git
cd mediarr

# Install frontend dependencies
cd frontend && npm install && cd ..

# Build the GUI (Tauri desktop app)
cargo tauri build

# Build the CLI only
cargo build --release -p mediarr-cli
```

After building the CLI, the binary is produced at `target/release/mediarr`.

## Quick Start

1. Build or run the CLI in development mode:
   ```bash
   cargo run -p mediarr-cli -- scan /path/to/media
   ```
2. Review the rename proposals printed to stdout.
3. Execute the renames:
   ```bash
   cargo run -p mediarr-cli -- rename /path/to/media
   ```
4. To launch the desktop GUI in development mode:
   ```bash
   cd frontend && npm install
   cargo tauri dev
   ```

## Usage

### CLI Commands

The CLI binary is named `mediarr` and provides these subcommands:

```
mediarr scan <path>       Scan a folder for media files and show rename proposals
mediarr rename <path>     Rename media files according to naming templates
mediarr history           Show rename history
mediarr undo <batch_id>   Undo a previous rename batch
mediarr watch <path>      Watch a folder for new media files
mediarr config            View or modify configuration
mediarr review            Review queued rename proposals from watch mode
```

### Scan and preview renames

```bash
mediarr scan ~/Downloads/Movies
```

This parses filenames using the `hunch` crate, identifies media metadata (title, year, season, episode), and shows what each file would be renamed to based on your naming templates.

Use `--tree` for a verbose view including subtitle details, or `--json` for machine-readable output:

```bash
mediarr scan ~/Downloads/Movies --tree
mediarr scan ~/Downloads/Movies --json
```

### Execute renames

```bash
mediarr rename ~/Downloads/TV --yes     # Skip confirmation prompt
mediarr rename ~/Downloads/TV --dry-run # Preview without executing
```

### Naming templates

Templates use `{variable}` syntax with optional modifiers. Defaults:

| Media Type | Default Template |
|------------|-----------------|
| Movie | `{Title} ({year})/{Title} ({year}).{ext}` |
| Series | `{Title}/{Title} - S{season:02}E{episode:02}.{ext}` |

Available variables: `title`, `Title`, `year`, `season`, `episode`, `ext`, `resolution`, `video_codec`, `audio_codec`, `source`, `release_group`, `language`.

The `:02` modifier zero-pads to 2 digits (e.g., `S01E03`).

### Watch mode

```bash
mediarr watch ~/Downloads/Media --mode auto        # Rename automatically
mediarr watch ~/Downloads/Media --mode review       # Queue for manual review
mediarr watch ~/Downloads/Media --debounce 10       # Custom debounce (seconds)
```

### Undo renames

Every rename batch is recorded in SQLite history. Undo by batch ID:

```bash
mediarr history
mediarr undo <batch_id>
```

## Architecture

Cargo workspace with three crates:

| Crate | Purpose |
|-------|---------|
| `mediarr-core` | Shared library containing all business logic (parsing, templates, scanning, renaming, history, watching, subtitles) |
| `mediarr-cli` | Thin CLI binary using `clap`, calls into mediarr-core |
| `mediarr-tauri` | Thin Tauri desktop shell, wraps mediarr-core functions as Tauri commands |

The frontend is a Svelte 5 + SvelteKit app in `frontend/`, using shadcn-svelte and TailwindCSS.

**Key design principles:**

- `mediarr-core` has zero knowledge of Tauri or any UI framework
- Subtitles are dependents of video files, never parsed independently
- Config is TOML, stored at the platform config directory (`dirs::config_dir()/mediarr/config.toml`)
- Rename history is SQLite at `dirs::data_dir()/mediarr/history.db`
- Source files are never deleted -- renames are moves or copies only

## Configuration

Mediarr uses a TOML config file at the platform-appropriate location:

- **macOS**: `~/Library/Application Support/mediarr/config.toml`
- **Linux**: `~/.config/mediarr/config.toml`
- **Windows**: `C:\Users\<User>\AppData\Roaming\mediarr\config.toml`

Manage config via CLI:

```bash
mediarr config --get templates.movie
mediarr config --set templates.movie "{Title} ({year})/{Title} ({year}).{ext}"
```

Both the GUI and CLI share the same config file.

## Development

```bash
# Run the GUI in dev mode
cd frontend && npm install
cargo tauri dev

# Run CLI in dev mode
cargo run -p mediarr-cli -- scan /path/to/folder

# Run all tests
cargo test --workspace

# Build release artifacts
cargo tauri build          # GUI
cargo build --release -p mediarr-cli  # CLI only
```

## License

This project does not currently include a LICENSE file. All rights reserved.

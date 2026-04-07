<!-- generated-by: gsd-doc-writer -->
# Configuration

Mediarr uses a single TOML configuration file shared by both the CLI and GUI. The file is loaded at startup and can be edited by hand or through the application.

## Config file location

The config file is stored at the platform-appropriate directory determined by the `dirs` crate:

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/mediarr/config.toml` |
| Linux | `~/.config/mediarr/config.toml` |
| Windows | `C:\Users\<User>\AppData\Roaming\mediarr\config.toml` |

The path is resolved by `mediarr_core::config::default_config_path()` using `dirs::config_dir()`. If the config file does not exist at startup, Mediarr uses built-in defaults. The file and its parent directories are created automatically when configuration is saved.

## Config file format

The config file is TOML with four top-level sections: `[general]`, `[templates]`, `[subtitles]`, and `[[watchers]]`.

A minimal working config file with all defaults explicitly written:

```toml
[general]
operation = "Move"
conflict_strategy = "Skip"
create_directories = true

[templates]
movie = "{Title} ({year})/{Title} ({year}).{ext}"
series = "{Title}/{Title} - S{season:02}E{episode:02}.{ext}"

[subtitles]
enabled = true
naming_pattern = "{video_name}.{lang}.{type}.{ext}"
preferred_languages = []
non_preferred_action = "Ignore"

[subtitles.discovery]
sidecar = true
subs_subfolder = true
nested_language_folders = true
vobsub_pairs = true
```

## Environment variables

Mediarr does not use environment variables for application configuration. All settings are stored in the TOML config file.

The only environment variable recognized is `RUST_LOG`, which controls the tracing/logging verbosity filter (standard `tracing_subscriber::EnvFilter` syntax). The CLI also supports `-v` / `-vv` / `-vvv` flags for verbosity.

## General settings

| Setting | Type | Required | Default | Description |
|---------|------|----------|---------|-------------|
| `general.output_dir` | String (path) | No | *none* (in-place rename) | Output directory for renamed files. When unset, files are renamed in their source directory. |
| `general.operation` | Enum | No | `Move` | Rename operation mode. Values: `Move`, `Copy`. |
| `general.conflict_strategy` | Enum | No | `Skip` | How to handle filename conflicts at the destination. Values: `Skip`, `Overwrite`, `NumericSuffix`. |
| `general.create_directories` | Boolean | No | `true` | Create target directories if they do not exist during rename. |

### Operation modes

- **Move** -- Renames (moves) the file. If the move crosses filesystem boundaries, Mediarr copies the file, verifies the copy by comparing file sizes, then removes the source. This is the only case where a source file is removed.
- **Copy** -- Copies the file to the destination, leaving the source in place.

### Conflict strategies

- **Skip** -- Leave the file unprocessed if the target path already exists.
- **Overwrite** -- Replace the existing file at the target path.
- **NumericSuffix** -- Append a numeric suffix: `file (1).ext`, `file (2).ext`, up to `(99)`.

## Template settings

| Setting | Type | Required | Default | Description |
|---------|------|----------|---------|-------------|
| `templates.movie` | String | No | `{Title} ({year})/{Title} ({year}).{ext}` | Naming template for movie files. |
| `templates.series` | String | No | `{Title}/{Title} - S{season:02}E{episode:02}.{ext}` | Naming template for series (TV and anime) files. |

### Template syntax

Templates use `{variable}` placeholders. Path separators (`/`) split the template into directory components. Each component is sanitized for cross-platform filesystem compatibility.

**Available variables:**

| Variable | Description | Example value |
|----------|-------------|---------------|
| `{title}` | Media title (raw case from parser) | `the matrix` |
| `{Title}` | Media title (title case) | `The Matrix` |
| `{year}` | Release year | `1999` |
| `{season}` | Season number | `1` |
| `{episode}` | Episode number(s) | `5` (multi-episode: `05E06E07`) |
| `{ext}` | File extension (without dot) | `mkv` |
| `{resolution}` | Video resolution | `1080p` |
| `{video_codec}` | Video codec | `x265` |
| `{audio_codec}` | Audio codec | `DTS-HD MA` |
| `{source}` | Source type | `BluRay` |
| `{release_group}` | Release group name | `SPARKS` |
| `{language}` | Content language | `English` |

**Format modifiers:**

Use `{variable:modifier}` for formatting. The supported modifier is zero-padding:

- `{season:02}` -- Zero-pad to 2 digits: `1` becomes `01`
- `{episode:02}` -- Zero-pad to 2 digits: `5` becomes `05`

**Multi-episode handling:**

When a file contains multiple episodes, `{episode:02}` renders all episode numbers joined by `E`: e.g., episodes 5, 6, 7 produce `05E06E07`.

**Dot collapsing:**

When a variable resolves to an empty string, adjacent dots are collapsed to prevent double dots in the output filename.

## Subtitle settings

| Setting | Type | Required | Default | Description |
|---------|------|----------|---------|-------------|
| `subtitles.enabled` | Boolean | No | `true` | Enable subtitle discovery during scans. |
| `subtitles.naming_pattern` | String | No | `{video_name}.{lang}.{type}.{ext}` | Template for subtitle output names. Variables: `{video_name}`, `{lang}`, `{type}`, `{ext}`. |
| `subtitles.preferred_languages` | Array of strings | No | `[]` (empty) | Ordered list of preferred ISO 639-1 language codes (e.g., `["en", "ja"]`). |
| `subtitles.non_preferred_action` | Enum | No | `Ignore` | What to do with subtitles not in the preferred languages list. Values: `Ignore`, `Backup`, `KeepAll`, `Review`. |
| `subtitles.backup_path` | String (path) | No | *none* | Backup directory for non-preferred subtitles when `non_preferred_action = "Backup"`. |

### Discovery toggles

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `subtitles.discovery.sidecar` | Boolean | `true` | Discover subtitle files in the same directory with matching filename stem. |
| `subtitles.discovery.subs_subfolder` | Boolean | `true` | Discover subtitles in `Subs/`, `Subtitles/`, or `Sub/` subfolders. |
| `subtitles.discovery.nested_language_folders` | Boolean | `true` | Discover subtitles in language-named subfolders (e.g., `English/`). |
| `subtitles.discovery.vobsub_pairs` | Boolean | `true` | Discover VobSub pairs (`.idx` + `.sub` files). |

### Non-preferred subtitle actions

- **Ignore** -- Leave non-preferred subtitles in place, unprocessed.
- **Backup** -- Move non-preferred subtitles to the path specified by `subtitles.backup_path`.
- **KeepAll** -- Rename all subtitles regardless of language preference.
- **Review** -- Flag non-preferred subtitles for user review.

### Recognised subtitle extensions

`srt`, `ass`, `ssa`, `sub`, `idx`, `sup`, `vtt`

## Watcher settings

Watchers are configured as a TOML array of tables (`[[watchers]]`). Each entry defines a folder to watch for new media files.

| Setting | Type | Required | Default | Description |
|---------|------|----------|---------|-------------|
| `watchers[].path` | String (path) | Yes | -- | Folder path to watch for new media files. |
| `watchers[].mode` | Enum | No | `auto` | Operating mode. Values: `auto` (scan and rename automatically), `review` (scan and queue for user review). |
| `watchers[].active` | Boolean | No | `true` | Whether this watcher starts automatically on application launch. |
| `watchers[].debounce_seconds` | Integer | No | `5` | Seconds to wait after the last filesystem event before processing. Accounts for files arriving progressively from torrent clients or browsers. |
| `watchers[].settings` | Table | No | *none* | Per-watcher setting overrides (see below). |

### Per-watcher setting overrides

Each watcher can override specific global settings. Fields not set in `settings` fall through to the corresponding global config value.

| Setting | Type | Overrides |
|---------|------|-----------|
| `settings.output_dir` | String | `general.output_dir` (empty string `""` forces in-place rename) |
| `settings.operation` | Enum | `general.operation` |
| `settings.conflict_strategy` | Enum | `general.conflict_strategy` |
| `settings.create_directories` | Boolean | `general.create_directories` |
| `settings.movie_template` | String | `templates.movie` |
| `settings.series_template` | String | `templates.series` |
| `settings.subtitles_enabled` | Boolean | `subtitles.enabled` |
| `settings.preferred_languages` | Array of strings | `subtitles.preferred_languages` |
| `settings.non_preferred_action` | Enum | `subtitles.non_preferred_action` |

Note: `subtitles.naming_pattern`, `subtitles.discovery.*`, and `subtitles.backup_path` are not overridable per-watcher and always use the global values.

### Example watcher configuration

```toml
[[watchers]]
path = "/media/downloads/movies"
mode = "auto"
active = true
debounce_seconds = 5

[[watchers]]
path = "/media/downloads/series"
mode = "review"
active = true
debounce_seconds = 10

[watchers.settings]
output_dir = "/media/library/series"
operation = "Copy"
series_template = "{Title}/Season {season:02}/{Title} - S{season:02}E{episode:02}.{ext}"
```

## Required vs optional settings

All config settings are optional. If the config file is missing or empty, Mediarr uses built-in defaults for all values. However, note that an **existing but incomplete** TOML file (e.g., containing `[general]` but missing `[templates]` and `[subtitles]`) will produce a parse error. Either omit the file entirely (all defaults) or provide all required sections.

The following settings cause startup errors if misconfigured:

| Condition | Error |
|-----------|-------|
| `dirs::config_dir()` returns `None` | `ConfigPathUnavailable` -- platform config directory cannot be determined |
| `dirs::data_dir()` returns `None` | `ConfigPathUnavailable` -- platform data directory cannot be determined |
| Malformed TOML syntax | `ConfigParse` -- TOML parse error with details |
| Missing required sections in an existing file | `ConfigParse` -- missing field error |

## Defaults

All default values in one place, as defined in the source code (`crates/mediarr-core/src/config.rs`):

| Setting | Default value |
|---------|--------------|
| `general.output_dir` | *none* (in-place rename) |
| `general.operation` | `Move` |
| `general.conflict_strategy` | `Skip` |
| `general.create_directories` | `true` |
| `templates.movie` | `{Title} ({year})/{Title} ({year}).{ext}` |
| `templates.series` | `{Title}/{Title} - S{season:02}E{episode:02}.{ext}` |
| `subtitles.enabled` | `true` |
| `subtitles.naming_pattern` | `{video_name}.{lang}.{type}.{ext}` |
| `subtitles.preferred_languages` | `[]` (empty) |
| `subtitles.non_preferred_action` | `Ignore` |
| `subtitles.backup_path` | *none* |
| `subtitles.discovery.sidecar` | `true` |
| `subtitles.discovery.subs_subfolder` | `true` |
| `subtitles.discovery.nested_language_folders` | `true` |
| `subtitles.discovery.vobsub_pairs` | `true` |
| `watchers` | `[]` (empty) |

## Per-environment overrides

Mediarr does not use per-environment config files (e.g., no `.env.development` or `.env.production`). There is a single config file per user, shared across all usage contexts.

To maintain different configurations for different purposes, use separate config files and specify the path explicitly via the CLI or by symlinking the config file location.

## CLI config management

The CLI provides a `config` subcommand for viewing and modifying settings without editing the TOML file directly:

```bash
# View full config
mediarr config

# Get a specific value using dotted path notation
mediarr config --get general.operation
mediarr config --get templates.movie
mediarr config --get subtitles.preferred_languages

# Set a value (type is inferred from the existing field type)
mediarr config --set general.operation Copy
mediarr config --set templates.movie "{Title} ({year}).{ext}"
mediarr config --set general.create_directories false
```

Enum fields (`operation`, `conflict_strategy`, `non_preferred_action`, `mode`) and template fields are validated before saving. Invalid values produce an error message with the allowed options.

## Data file location

Rename history is stored in a SQLite database at the platform data directory:

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/mediarr/history.db` |
| Linux | `~/.local/share/mediarr/history.db` |
| Windows | `C:\Users\<User>\AppData\Roaming\mediarr\history.db` |

The path is resolved by `mediarr_core::config::default_data_path()` using `dirs::data_dir()`. The database file and parent directories are created automatically on first use.

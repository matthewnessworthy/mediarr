//! Integration tests for the mediarr CLI binary.
//!
//! Uses `assert_cmd` to run the compiled binary and verify behavior
//! of all major subcommands end-to-end.

use assert_cmd::Command;
use assert_fs::prelude::*;

/// Helper to get a Command for the mediarr binary.
fn mediarr() -> Command {
    Command::cargo_bin("mediarr").expect("binary should be built")
}

/// Helper to create a Command with HOME set to a temp directory,
/// with a config file that has output_dir set to the given path.
///
/// Uses platform-appropriate config/data paths so this works on macOS, Linux, and Windows CI.
fn mediarr_with_config(fake_home: &std::path::Path, output_dir: &std::path::Path) -> Command {
    // Determine platform-appropriate config and data directories.
    // On macOS: $HOME/Library/Application Support/mediarr
    // On Linux: $XDG_CONFIG_HOME/mediarr (we set XDG vars to force the path)
    // On Windows: $APPDATA/mediarr
    let config_dir;
    let data_dir;

    #[cfg(target_os = "macos")]
    {
        config_dir = fake_home.join("Library/Application Support/mediarr");
        data_dir = fake_home.join("Library/Application Support/mediarr");
    }
    #[cfg(target_os = "linux")]
    {
        config_dir = fake_home.join(".config/mediarr");
        data_dir = fake_home.join(".local/share/mediarr");
    }
    #[cfg(target_os = "windows")]
    {
        config_dir = fake_home.join("AppData/Roaming/mediarr");
        data_dir = fake_home.join("AppData/Local/mediarr");
    }

    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::create_dir_all(&data_dir).unwrap();

    std::fs::write(
        config_dir.join("config.toml"),
        format!(
            r#"[general]
output_dir = "{}"
operation = "Move"
conflict_strategy = "Skip"
create_directories = true

[templates]
movie = "{{Title}} ({{year}})/{{Title}} ({{year}}).{{ext}}"
series = "{{title}}/Season {{season:02}}/{{title}} - S{{season:02}}E{{episode:02}}.{{ext}}"

[subtitles]
enabled = true
preferred_languages = ["en"]

[subtitles.discovery]
sidecar = true
subs_subfolder = true
nested_language_folders = true
vobsub_pairs = true
"#,
            output_dir.display().to_string().replace('\\', "/")
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("mediarr").expect("binary should be built");
    cmd.env("HOME", fake_home);
    // On Linux, dirs uses XDG vars; on Windows, APPDATA/LOCALAPPDATA
    #[cfg(target_os = "linux")]
    {
        cmd.env("XDG_CONFIG_HOME", fake_home.join(".config"));
        cmd.env("XDG_DATA_HOME", fake_home.join(".local/share"));
    }
    #[cfg(target_os = "windows")]
    {
        cmd.env("APPDATA", fake_home.join("AppData/Roaming"));
        cmd.env("LOCALAPPDATA", fake_home.join("AppData/Local"));
    }
    cmd
}

// -----------------------------------------------------------------------
// Test 1: no_args_shows_help
// -----------------------------------------------------------------------

#[test]
fn no_args_shows_help() {
    mediarr()
        .assert()
        .failure()
        .stderr(predicates::str::contains("Usage"));
}

// -----------------------------------------------------------------------
// Test 2: scan_shows_table_output
// -----------------------------------------------------------------------

#[test]
fn scan_shows_table_output() {
    let dir = assert_fs::TempDir::new().unwrap();
    dir.child("The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv")
        .write_binary(b"fake video data")
        .unwrap();

    mediarr()
        .arg("scan")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Office"));
}

// -----------------------------------------------------------------------
// Test 3: scan_json_output_is_valid_json
// -----------------------------------------------------------------------

#[test]
fn scan_json_output_is_valid_json() {
    let dir = assert_fs::TempDir::new().unwrap();
    dir.child("The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv")
        .write_binary(b"fake video data")
        .unwrap();

    let output = mediarr()
        .arg("scan")
        .arg("--json")
        .arg(dir.path())
        .output()
        .expect("command should run");

    assert!(output.status.success(), "scan --json should succeed");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert!(parsed.is_array(), "JSON output should be an array");

    let arr = parsed.as_array().unwrap();
    assert!(!arr.is_empty(), "should have at least one scan result");
    assert!(
        arr[0].get("source_path").is_some(),
        "entry should have source_path"
    );
    assert!(
        arr[0].get("media_info").is_some(),
        "entry should have media_info"
    );
}

// -----------------------------------------------------------------------
// Test 4: scan_type_filter
// -----------------------------------------------------------------------

#[test]
fn scan_type_filter() {
    let dir = assert_fs::TempDir::new().unwrap();
    dir.child("Inception.2010.1080p.BluRay.mkv")
        .write_binary(b"movie data")
        .unwrap();
    dir.child("The.Office.S01E01.720p.mkv")
        .write_binary(b"series data")
        .unwrap();

    // Filter to movie type -- should succeed
    mediarr()
        .arg("scan")
        .arg("--type")
        .arg("movie")
        .arg(dir.path())
        .assert()
        .success();
}

// -----------------------------------------------------------------------
// Test 5: scan_nonexistent_path_errors
// -----------------------------------------------------------------------

#[test]
fn scan_nonexistent_path_errors() {
    mediarr()
        .arg("scan")
        .arg("/nonexistent/path/that/does/not/exist")
        .assert()
        .failure();
}

// -----------------------------------------------------------------------
// Test 6: history_exits_ok
// -----------------------------------------------------------------------

#[test]
fn history_exits_ok() {
    // History should exit 0 regardless of whether there's data
    // (prints "No rename history found" to stderr if empty)
    mediarr().arg("history").assert().success();
}

// -----------------------------------------------------------------------
// Test 7: config_no_args_shows_full_config
// -----------------------------------------------------------------------

#[test]
fn config_no_args_shows_full_config() {
    mediarr()
        .arg("config")
        .assert()
        .success()
        .stdout(predicates::str::contains("[general]"))
        .stdout(predicates::str::contains("[templates]"))
        .stdout(predicates::str::contains("[subtitles]"));
}

// -----------------------------------------------------------------------
// Test 8: undo_nonexistent_batch_id
// -----------------------------------------------------------------------

#[test]
fn undo_nonexistent_batch_id() {
    // Undo with a batch that has zero entries exits 0 (no files to undo)
    // but prints the batch info. Verify it at least runs without crashing.
    mediarr()
        .arg("undo")
        .arg("nonexistent-batch-id-12345")
        .assert()
        .success()
        .stderr(predicates::str::contains("Undo"));
}

// -----------------------------------------------------------------------
// Test 9: scan_tree_shows_detail
// -----------------------------------------------------------------------

#[test]
fn scan_tree_shows_detail() {
    let dir = assert_fs::TempDir::new().unwrap();
    dir.child("The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv")
        .write_binary(b"fake video data")
        .unwrap();

    mediarr()
        .arg("scan")
        .arg("--tree")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv",
        ));
}

// -----------------------------------------------------------------------
// Test 10: rename_moves_files_to_output_dir
// -----------------------------------------------------------------------

#[test]
fn rename_moves_files_to_output_dir() {
    let source_dir = assert_fs::TempDir::new().unwrap();
    let output_dir = assert_fs::TempDir::new().unwrap();
    let fake_home = assert_fs::TempDir::new().unwrap();

    // Create a video file with a recognizable series name
    source_dir
        .child("The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv")
        .write_binary(b"fake video data")
        .unwrap();

    // Run rename with --yes to auto-confirm
    mediarr_with_config(fake_home.path(), output_dir.path())
        .arg("rename")
        .arg("--yes")
        .arg(source_dir.path())
        .assert()
        .success()
        .stderr(predicates::str::contains("Renamed 1 files"));

    // Verify source file is gone (Move operation)
    assert!(
        !source_dir
            .path()
            .join("The.Office.S02E03.720p.BluRay.x264-DEMAND.mkv")
            .exists(),
        "Source file should be moved"
    );

    // Verify a file appeared somewhere under output_dir by walking recursively
    fn find_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    files.extend(find_files(&path));
                } else {
                    files.push(path);
                }
            }
        }
        files
    }

    let output_files = find_files(output_dir.path());
    assert_eq!(
        output_files.len(),
        1,
        "Should have exactly 1 file in output dir, found: {:?}",
        output_files
    );

    let output_file = &output_files[0];
    assert!(
        output_file.extension().map(|e| e == "mkv").unwrap_or(false),
        "Output file should have .mkv extension: {:?}",
        output_file
    );
}

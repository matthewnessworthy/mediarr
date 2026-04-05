//! Integration tests for the mediarr CLI binary.
//!
//! Uses `assert_cmd` to run the compiled binary and verify behavior
//! of all major subcommands end-to-end.

use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

/// Helper to get a Command for the mediarr binary.
fn mediarr() -> Command {
    Command::cargo_bin("mediarr").expect("binary should be built")
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

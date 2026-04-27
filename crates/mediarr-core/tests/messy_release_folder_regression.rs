//! Regression tests for movie scanning when the source file lives inside a
//! messy release-group folder whose name contains the year *and* extra metadata
//! (resolution, source, group tag, etc).
//!
//! Bug history:
//! - Phase 11 (parent-folder-context-inheritance) introduced two regressions:
//!   1. Sibling-aware parsing (`parse_with_context`) was called with the file
//!      itself in the sibling list. Hunch's cross-file invariance treats shared
//!      tokens (like the year) as part of the title, so single-file directories
//!      produced titles like "They Will Kill You 2026" instead of
//!      "They Will Kill You".
//!   2. `in_place_proposed_path` only avoided re-nesting via raw-string
//!      equality. Messy release folders (`Title (Year) [1080p] ... [GROUP]`)
//!      do not equal the rendered template folder (`Title (Year)`), so output
//!      was nested inside the messy folder instead of replacing it.
//!
//! Both bugs surface together for YTS-style downloads. These tests pin the
//! contract going forward.

use mediarr_core::config::Config;
use mediarr_core::scanner::Scanner;
use std::fs;
use tempfile::TempDir;

const RELEASE_FOLDER: &str = "They Will Kill You (2026) [1080p] [WEBRip] [5.1] [YTS.BZ]";
const RELEASE_FILE: &str = "They.Will.Kill.You.2026.1080p.WEBRip.x264.AAC5.1-[YTS.BZ].mp4";

#[test]
fn scan_folder_messy_release_folder_has_clean_title_no_year_suffix() {
    let source = TempDir::new().unwrap();
    let release_folder = source.path().join(RELEASE_FOLDER);
    fs::create_dir(&release_folder).unwrap();
    fs::write(release_folder.join(RELEASE_FILE), b"video").unwrap();

    let scanner = Scanner::new(Config::default()); // in-place mode
    let results = scanner.scan_folder(source.path()).unwrap();
    assert_eq!(results.len(), 1);
    let r = &results[0];

    assert_eq!(
        r.media_info.title, "They Will Kill You",
        "title must not carry the year as a suffix"
    );
    // Defense-in-depth: explicit guard against year-leaking-into-title regressions.
    assert!(
        !r.media_info.title.contains("2026"),
        "title must not contain the year token, got: {:?}",
        r.media_info.title
    );
    assert_eq!(r.media_info.year, Some(2026));
}

#[test]
fn scan_folder_messy_release_folder_replaces_parent_with_clean_folder() {
    let source = TempDir::new().unwrap();
    let release_folder = source.path().join(RELEASE_FOLDER);
    fs::create_dir(&release_folder).unwrap();
    fs::write(release_folder.join(RELEASE_FILE), b"video").unwrap();

    let scanner = Scanner::new(Config::default());
    let results = scanner.scan_folder(source.path()).unwrap();
    let r = &results[0];

    let path_str = r.proposed_path.to_string_lossy();
    assert!(
        !path_str.contains("[YTS.BZ]") && !path_str.contains("[1080p]"),
        "proposed_path must not nest under the messy release folder, got: {}",
        path_str
    );

    // Expected layout: <source-root>/They Will Kill You (2026)/They Will Kill You (2026).mp4
    let components: Vec<_> = r.proposed_path.components().collect();
    let len = components.len();
    let folder = components[len - 2].as_os_str().to_str().unwrap();
    let file = components[len - 1].as_os_str().to_str().unwrap();
    assert_eq!(folder, "They Will Kill You (2026)");
    assert_eq!(file, "They Will Kill You (2026).mp4");
}

#[test]
fn scan_file_messy_release_folder_has_clean_title_no_year_suffix() {
    let source = TempDir::new().unwrap();
    let release_folder = source.path().join(RELEASE_FOLDER);
    fs::create_dir(&release_folder).unwrap();
    let video = release_folder.join(RELEASE_FILE);
    fs::write(&video, b"video").unwrap();

    let scanner = Scanner::new(Config::default());
    let r = scanner.scan_file(&video).unwrap();

    assert_eq!(r.media_info.title, "They Will Kill You");
    assert!(
        !r.media_info.title.contains("2026"),
        "title must not contain the year token, got: {:?}",
        r.media_info.title
    );
    assert_eq!(r.media_info.year, Some(2026));
}

#[test]
fn scan_file_messy_release_folder_replaces_parent_with_clean_folder() {
    let source = TempDir::new().unwrap();
    let release_folder = source.path().join(RELEASE_FOLDER);
    fs::create_dir(&release_folder).unwrap();
    let video = release_folder.join(RELEASE_FILE);
    fs::write(&video, b"video").unwrap();

    let scanner = Scanner::new(Config::default());
    let r = scanner.scan_file(&video).unwrap();

    let path_str = r.proposed_path.to_string_lossy();
    assert!(
        !path_str.contains("[YTS.BZ]") && !path_str.contains("[1080p]"),
        "proposed_path must not nest under the messy release folder, got: {}",
        path_str
    );

    let components: Vec<_> = r.proposed_path.components().collect();
    let len = components.len();
    let folder = components[len - 2].as_os_str().to_str().unwrap();
    let file = components[len - 1].as_os_str().to_str().unwrap();
    assert_eq!(folder, "They Will Kill You (2026)");
    assert_eq!(file, "They Will Kill You (2026).mp4");
}

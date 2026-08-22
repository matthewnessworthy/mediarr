//! Regression tests for series naming defects in post-parse normalisation.
//!
//! Bug history:
//! - Bug B: sibling-aware parsing (`parse_with_context`) glues a token shared by
//!   every sibling into the title. For `lioness (2016)/lioness.2016.s03e01.mkv`
//!   plus siblings, hunch's cross-file invariance keeps the year inside the
//!   title (`lioness 2016`), so the `{year}` template variable renders the year
//!   a second time (`Lioness 2016 (2016)`). Only reproduces with **two or more**
//!   video siblings -- with a single file the Phase-11 self-exclusion in
//!   `Scanner::scan_folder` already suppresses it.
//! - Bug A: a bare single-digit season-like token at the end of a series title
//!   (`Fumetsu no Anata e S3`) was consumed as a season number and stripped from
//!   the title, so the show lost its name and the file was re-nested one level
//!   deeper instead of being renamed in place.
//!
//! Both are worked around in `mediarr-core`'s post-parse normalisation --
//! the `hunch` crate is never patched.

use mediarr_core::config::Config;
use mediarr_core::scanner::Scanner;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Return the last two components of a path as `(parent_dir, file_name)`.
fn last_two_components(path: &Path) -> (String, String) {
    let components: Vec<_> = path.components().collect();
    let len = components.len();
    let folder = components[len - 2].as_os_str().to_str().unwrap().to_owned();
    let file = components[len - 1].as_os_str().to_str().unwrap().to_owned();
    (folder, file)
}

// ---------------------------------------------------------------------------
// Bug B -- duplicated year token in the title
// ---------------------------------------------------------------------------

const LIONESS_FOLDER: &str = "lioness (2016)";
const LIONESS_E01: &str = "lioness.2016.s03e01.mkv";
const LIONESS_E02: &str = "lioness.2016.s03e02.mkv";

/// Build `<tmp>/lioness (2016)/{lioness.2016.s03e01.mkv, lioness.2016.s03e02.mkv}`.
///
/// Two sibling episodes are mandatory: a single file does not reproduce Bug B.
fn lioness_fixture() -> TempDir {
    let source = TempDir::new().unwrap();
    let series_dir = source.path().join(LIONESS_FOLDER);
    fs::create_dir(&series_dir).unwrap();
    fs::write(series_dir.join(LIONESS_E01), b"ep1").unwrap();
    fs::write(series_dir.join(LIONESS_E02), b"ep2").unwrap();
    source
}

#[test]
fn scan_folder_series_title_drops_duplicated_year_token() {
    let source = lioness_fixture();

    let scanner = Scanner::new(Config::default()); // in-place mode
    let results = scanner.scan_folder(source.path()).unwrap();
    assert_eq!(results.len(), 2, "both sibling episodes should be scanned");

    for r in &results {
        assert_eq!(
            r.media_info.title, "lioness",
            "title must not carry the year as a suffix, got: {:?}",
            r.media_info.title
        );
        assert_eq!(r.media_info.year, Some(2016));
    }
}

#[test]
fn scan_folder_series_proposed_path_renders_year_once() {
    let source = lioness_fixture();

    let scanner = Scanner::new(Config::default());
    let results = scanner.scan_folder(source.path()).unwrap();

    let e01 = results
        .iter()
        .find(|r| r.source_path.file_name().unwrap() == LIONESS_E01)
        .expect("episode 01 result");

    let (folder, file) = last_two_components(&e01.proposed_path);
    assert_eq!(folder, "Lioness (2016)");
    assert_eq!(file, "Lioness (2016) - S03E01.mkv");
}

// ---------------------------------------------------------------------------
// Bug A -- bare single-digit season-like token at the end of a series name
// ---------------------------------------------------------------------------

const FUMETSU_FOLDER: &str = "Fumetsu no Anata e S3";
const FUMETSU_E01: &str = "[SubsPlease] Fumetsu no Anata e S3 - 01 (1080p) [A1B2C3D4].mkv";
const FUMETSU_E02: &str = "[SubsPlease] Fumetsu no Anata e S3 - 02 (1080p) [E5F6A7B8].mkv";

/// Build `<tmp>/Fumetsu no Anata e S3/{episode 01, episode 02}`.
fn fumetsu_fixture() -> TempDir {
    let source = TempDir::new().unwrap();
    let series_dir = source.path().join(FUMETSU_FOLDER);
    fs::create_dir(&series_dir).unwrap();
    fs::write(series_dir.join(FUMETSU_E01), b"ep1").unwrap();
    fs::write(series_dir.join(FUMETSU_E02), b"ep2").unwrap();
    source
}

#[test]
fn scan_folder_keeps_bare_season_token_in_series_title() {
    let source = fumetsu_fixture();

    let scanner = Scanner::new(Config::default()); // in-place mode
    let results = scanner.scan_folder(source.path()).unwrap();
    assert_eq!(results.len(), 2, "both sibling episodes should be scanned");

    let e01 = results
        .iter()
        .find(|r| r.source_path.file_name().unwrap() == FUMETSU_E01)
        .expect("episode 01 result");

    assert_eq!(
        e01.media_info.title, "Fumetsu no Anata e S3",
        "the bare 'S3' token belongs to the show's name"
    );
    assert_eq!(e01.media_info.episodes, vec![1]);
    // D-A1: the bare token yields no season, so the pre-existing
    // "episode present, season missing -> season 1" default applies.
    assert_eq!(e01.media_info.season, Some(1));
}

#[test]
fn scan_folder_bare_season_token_renames_in_place_without_renesting() {
    let source = fumetsu_fixture();

    let scanner = Scanner::new(Config::default());
    let results = scanner.scan_folder(source.path()).unwrap();

    let e01 = results
        .iter()
        .find(|r| r.source_path.file_name().unwrap() == FUMETSU_E01)
        .expect("episode 01 result");

    // The folder component proves the raw-string arm of in_place_proposed_path
    // matches once the title is correct: the file is renamed inside its
    // existing directory rather than nested one level deeper.
    let (folder, file) = last_two_components(&e01.proposed_path);
    assert_eq!(folder, "Fumetsu no Anata e S3");
    assert_eq!(file, "Fumetsu no Anata e S3 - S01E01.mkv");
}

#[test]
fn scan_file_keeps_bare_season_token_in_series_title() {
    // The watcher enters through scan_file, not scan_folder.
    let source = fumetsu_fixture();
    let video = source.path().join(FUMETSU_FOLDER).join(FUMETSU_E01);

    let scanner = Scanner::new(Config::default());
    let r = scanner.scan_file(&video).unwrap();

    assert_eq!(r.media_info.title, "Fumetsu no Anata e S3");
    assert_eq!(r.media_info.episodes, vec![1]);
    assert_eq!(r.media_info.season, Some(1));
}

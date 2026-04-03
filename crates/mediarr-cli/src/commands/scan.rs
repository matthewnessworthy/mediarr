//! Scan command implementation.
//!
//! Thin wrapper around `mediarr_core::Scanner` that formats output
//! as a table, verbose tree view, or JSON.

use mediarr_core::{Config, MediaType, ScanFilter, ScanStatus, Scanner};

use crate::output::OutputFormatter;
use crate::ScanArgs;

/// Execute the scan command.
pub async fn execute(args: ScanArgs) -> anyhow::Result<()> {
    // Load config from default path (returns defaults if file missing)
    let config_path = mediarr_core::config::default_config_path()?;
    let config = Config::load(&config_path)?;

    // Create scanner and scan
    let scanner = Scanner::new(config);
    let results = scanner.scan_folder(&args.path)?;

    // Apply media type filter if specified
    let display_results: Vec<_> = if let Some(ref media_type_str) = args.media_type {
        let media_type = match media_type_str.as_str() {
            "movie" => MediaType::Movie,
            "series" => MediaType::Series,
            "anime" => MediaType::Anime,
            _ => unreachable!("clap validates this"),
        };
        let filter = ScanFilter {
            media_type: Some(media_type),
            ..ScanFilter::default()
        };
        Scanner::filter_results(&results, &filter)
            .into_iter()
            .cloned()
            .collect()
    } else {
        results.clone()
    };

    // Format output
    let formatter = OutputFormatter::new(args.json);

    if args.json {
        formatter.scan_json(&display_results);
    } else if args.verbose {
        formatter.scan_verbose(&display_results);
    } else {
        formatter.scan_table(&display_results);
    }

    // Summary to stderr
    let ok_count = display_results
        .iter()
        .filter(|r| r.status == ScanStatus::Ok)
        .count();
    let ambiguous_count = display_results
        .iter()
        .filter(|r| r.status == ScanStatus::Ambiguous)
        .count();
    let conflict_count = display_results
        .iter()
        .filter(|r| r.status == ScanStatus::Conflict || r.status == ScanStatus::Error)
        .count();

    formatter.print_summary(&format!(
        "Found {} files ({} ready, {} ambiguous, {} conflicts)",
        display_results.len(),
        ok_count,
        ambiguous_count,
        conflict_count
    ));

    Ok(())
}

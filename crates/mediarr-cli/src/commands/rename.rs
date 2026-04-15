//! Rename command implementation.
//!
//! Scans a folder, presents a rename plan, and executes renames with
//! history recording. Supports dry-run and auto-confirm (--yes) modes.

use std::io::{self, BufRead, Write};

use mediarr_core::{Config, HistoryDb, RenamePlan, RenamePlanEntry, Renamer, ScanStatus, Scanner};

use crate::output::OutputFormatter;
use crate::RenameArgs;

/// Execute the rename command.
pub async fn execute(args: RenameArgs) -> anyhow::Result<()> {
    // Load config
    let config_path = mediarr_core::config::default_config_path()?;
    let config = Config::load(&config_path)?;

    // Scan the folder
    let scanner = Scanner::new(config.clone());
    let results = scanner.scan_folder(&args.path)?;

    // Filter to actionable results (Ok and Ambiguous only)
    let actionable: Vec<_> = results
        .iter()
        .filter(|r| r.status == ScanStatus::Ok || r.status == ScanStatus::Ambiguous)
        .collect();

    if actionable.is_empty() {
        eprintln!("No files to rename");
        return Ok(());
    }

    // Build numbered list
    let numbered: Vec<(usize, &mediarr_core::ScanResult)> = actionable
        .iter()
        .enumerate()
        .map(|(i, r)| (i + 1, *r))
        .collect();

    let formatter = OutputFormatter::new(false);

    // Dry-run mode: show plan and exit
    if args.dry_run {
        formatter.rename_plan(&numbered);
        eprintln!("Dry run -- no files renamed");
        return Ok(());
    }

    // Show plan
    formatter.rename_plan(&numbered);

    // Confirmation
    let selected = if args.yes {
        // Auto-confirm all
        actionable.clone()
    } else {
        // Interactive confirmation
        eprint!("Enter numbers to exclude (comma-separated), or press Enter to rename all, q to abort: ");
        io::stderr().flush()?;

        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        let input = input.trim();

        if input.eq_ignore_ascii_case("q") {
            eprintln!("Aborted");
            return Ok(());
        }

        if input.is_empty() {
            actionable.clone()
        } else {
            // Parse exclusion numbers
            let excludes: Vec<usize> = input
                .split(',')
                .filter_map(|s| s.trim().parse::<usize>().ok())
                .collect();

            actionable
                .iter()
                .enumerate()
                .filter(|(i, _)| !excludes.contains(&(i + 1)))
                .map(|(_, r)| *r)
                .collect()
        }
    };

    if selected.is_empty() {
        eprintln!("No files selected for rename");
        return Ok(());
    }

    // Build rename plan from selected results (video files + subtitles)
    let mut plan_entries = Vec::new();
    for r in &selected {
        plan_entries.push(RenamePlanEntry {
            source_path: r.source_path.clone(),
            dest_path: r.proposed_path.clone(),
        });
        for sub in &r.subtitles {
            plan_entries.push(RenamePlanEntry {
                source_path: sub.source_path.clone(),
                dest_path: sub.proposed_path.clone(),
            });
        }
    }

    let plan = RenamePlan {
        entries: plan_entries,
    };

    // Execute renames
    let renamer = Renamer::from_config(&config.general);
    let results = renamer.execute(&plan);

    // Display results
    formatter.rename_results(&results);

    // Record to history
    if results.iter().any(|r| r.success) {
        let data_path = mediarr_core::config::default_data_path()?;
        let db = HistoryDb::open(&data_path)?;

        let media_info_map: std::collections::HashMap<String, mediarr_core::MediaInfo> = selected
            .iter()
            .map(|r| {
                (
                    r.source_path.to_string_lossy().to_string(),
                    r.media_info.clone(),
                )
            })
            .collect();

        let batch_id = db.record_rename_results(&results, &media_info_map)?;
        if !batch_id.is_empty() {
            eprintln!("Batch {batch_id} recorded to history");
        }
    }

    // Summary
    let success_count = results.iter().filter(|r| r.success).count();
    let fail_count = results.iter().filter(|r| !r.success).count();
    formatter.print_summary(&format!(
        "Renamed {} files, {} failed",
        success_count, fail_count
    ));

    // Return error with context for exit code handling in main
    if fail_count > 0 && success_count > 0 {
        anyhow::bail!("partial rename failure: {success_count} succeeded, {fail_count} failed");
    } else if fail_count > 0 {
        anyhow::bail!("all {fail_count} renames failed");
    }

    Ok(())
}

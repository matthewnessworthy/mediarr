//! Review command implementation.
//!
//! Shows pending rename proposals from watch mode's review queue.
//! Supports --approve-all, --reject-all, and interactive selection
//! with stale file detection.

use std::io::{self, BufRead, Write};

use mediarr_core::{
    Config, HistoryDb, RenamePlan, RenamePlanEntry, RenameRecord, Renamer, ReviewStatus,
};

use crate::ReviewArgs;

/// Execute the review command.
pub fn execute(args: ReviewArgs) -> anyhow::Result<()> {
    let config_path = mediarr_core::config::default_config_path()?;
    let config = Config::load(&config_path)?;

    let data_path = mediarr_core::config::default_data_path()?;
    let db = HistoryDb::open(&data_path)?;

    let pending = db.list_review_queue(None, Some(ReviewStatus::Pending))?;

    if pending.is_empty() {
        println!("No files pending review");
        return Ok(());
    }

    // Check for stale entries (source file no longer exists)
    let stale_flags: Vec<bool> = pending.iter().map(|e| !e.source_path.exists()).collect();

    if args.approve_all {
        approve_all(&pending, &stale_flags, &config, &db)?;
    } else if args.reject_all {
        reject_all(&pending, &db)?;
    } else {
        interactive_review(&pending, &stale_flags, &config, &db)?;
    }

    Ok(())
}

/// Approve all non-stale entries, reject stale ones.
fn approve_all(
    entries: &[mediarr_core::ReviewQueueEntry],
    stale_flags: &[bool],
    config: &Config,
    db: &HistoryDb,
) -> anyhow::Result<()> {
    let renamer = Renamer::from_config(&config.general);
    let mut approved = 0;
    let mut rejected_stale = 0;

    for (i, entry) in entries.iter().enumerate() {
        let id = entry.id.unwrap_or(0);

        if stale_flags[i] {
            db.update_review_status(id, ReviewStatus::Rejected)?;
            eprintln!(
                "Rejected (stale): {}",
                entry
                    .source_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            );
            rejected_stale += 1;
            continue;
        }

        // Execute rename
        if let Err(e) = execute_review_rename(entry, &renamer, db) {
            eprintln!(
                "Failed: {} - {e}",
                entry
                    .source_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            );
            continue;
        }

        db.update_review_status(id, ReviewStatus::Approved)?;
        println!(
            "Approved: {} -> {}",
            entry
                .source_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            entry.proposed_path.display()
        );
        approved += 1;
    }

    eprintln!("Approved {approved} files, rejected {rejected_stale} stale entries");
    Ok(())
}

/// Reject all pending entries.
fn reject_all(entries: &[mediarr_core::ReviewQueueEntry], db: &HistoryDb) -> anyhow::Result<()> {
    for entry in entries {
        let id = entry.id.unwrap_or(0);
        db.update_review_status(id, ReviewStatus::Rejected)?;
    }
    eprintln!("Rejected {} entries", entries.len());
    Ok(())
}

/// Interactive review: show numbered list, accept user input.
fn interactive_review(
    entries: &[mediarr_core::ReviewQueueEntry],
    stale_flags: &[bool],
    config: &Config,
    db: &HistoryDb,
) -> anyhow::Result<()> {
    // Show numbered list
    println!("Pending review:");
    println!();
    for (i, entry) in entries.iter().enumerate() {
        let stale = if stale_flags[i] { " [STALE]" } else { "" };
        let source_name = entry
            .source_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        println!(
            "  {}. {}{} -> {}",
            i + 1,
            source_name,
            stale,
            entry.proposed_path.display()
        );
    }
    println!();

    // Prompt
    eprint!("Enter numbers to approve (comma-separated), 'r' to reject all, or 'q' to quit: ");
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    let input = input.trim();

    if input.eq_ignore_ascii_case("q") {
        eprintln!("Aborted");
        return Ok(());
    }

    if input.eq_ignore_ascii_case("r") {
        reject_all(entries, db)?;
        return Ok(());
    }

    // Parse selection numbers
    let selected: Vec<usize> = input
        .split(',')
        .filter_map(|s| s.trim().parse::<usize>().ok())
        .filter(|n| *n >= 1 && *n <= entries.len())
        .collect();

    if selected.is_empty() {
        eprintln!("No valid entries selected");
        return Ok(());
    }

    let renamer = Renamer::from_config(&config.general);
    let mut approved = 0;

    for idx in &selected {
        let i = idx - 1;
        let entry = &entries[i];
        let id = entry.id.unwrap_or(0);

        if stale_flags[i] {
            eprintln!(
                "Skipped (stale): {}",
                entry
                    .source_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            );
            db.update_review_status(id, ReviewStatus::Rejected)?;
            continue;
        }

        if let Err(e) = execute_review_rename(entry, &renamer, db) {
            eprintln!(
                "Failed: {} - {e}",
                entry
                    .source_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            );
            continue;
        }

        db.update_review_status(id, ReviewStatus::Approved)?;
        println!(
            "Approved: {} -> {}",
            entry
                .source_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            entry.proposed_path.display()
        );
        approved += 1;
    }

    eprintln!("Approved {approved} files");
    Ok(())
}

/// Execute a single rename from a review queue entry and record to history.
fn execute_review_rename(
    entry: &mediarr_core::ReviewQueueEntry,
    renamer: &Renamer,
    db: &HistoryDb,
) -> anyhow::Result<()> {
    let plan = RenamePlan {
        entries: vec![RenamePlanEntry {
            source_path: entry.source_path.clone(),
            dest_path: entry.proposed_path.clone(),
        }],
    };

    let results = renamer.execute(&plan);
    let all_success = results.iter().all(|r| r.success);

    if !all_success {
        let errors: Vec<String> = results
            .iter()
            .filter(|r| !r.success)
            .filter_map(|r| r.error.clone())
            .collect();
        anyhow::bail!("rename failed: {}", errors.join("; "));
    }

    // Record to history
    let batch_id = HistoryDb::generate_batch_id();
    let timestamp = chrono::Utc::now().to_rfc3339();

    let media_info: mediarr_core::MediaInfo =
        serde_json::from_str(&entry.media_info_json).unwrap_or_default();

    let meta = std::fs::metadata(&entry.proposed_path).ok();
    let file_size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let file_mtime = meta
        .and_then(|m| m.modified().ok())
        .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
        .unwrap_or_default();

    let records = vec![RenameRecord {
        batch_id,
        timestamp,
        source_path: entry.source_path.clone(),
        dest_path: entry.proposed_path.clone(),
        media_info,
        file_size,
        file_mtime,
    }];

    db.record_batch(&records)?;
    Ok(())
}

//! Undo command implementation.
//!
//! Reverses a rename batch by moving files back to their original
//! locations. Checks eligibility before executing.

use mediarr_core::HistoryDb;

use crate::output::OutputFormatter;
use crate::UndoArgs;

/// Execute the undo command.
pub fn execute(args: UndoArgs) -> anyhow::Result<()> {
    let data_path = mediarr_core::config::default_data_path()?;
    let db = HistoryDb::open(&data_path)?;

    // Check eligibility first
    let eligibility = db.check_undo_eligible(&args.batch_id)?;

    if !eligibility.eligible {
        let issues: Vec<String> = eligibility
            .ineligible_reasons
            .iter()
            .map(|issue| format!("  - {}: {}", issue.dest_path.display(), issue.reason))
            .collect();
        anyhow::bail!(
            "Batch {} is not eligible for undo:\n{}",
            args.batch_id,
            issues.join("\n")
        );
    }

    // Show what will be undone
    let batch_entries = db.get_batch(&args.batch_id)?;
    eprintln!(
        "Undoing batch {} ({} files):",
        &args.batch_id[..8.min(args.batch_id.len())],
        batch_entries.len()
    );
    for entry in &batch_entries {
        eprintln!(
            "  {} -> {}",
            entry.dest_path.display(),
            entry.source_path.display()
        );
    }

    // Execute undo
    let results = db.execute_undo(&args.batch_id)?;

    let formatter = OutputFormatter::new(false);
    formatter.undo_results(&results);

    let success_count = results.iter().filter(|r| r.success).count();
    formatter.print_summary(&format!("Undo complete: {} files restored", success_count));

    Ok(())
}

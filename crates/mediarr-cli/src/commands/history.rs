//! History command implementation.
//!
//! Lists rename batches from the history database. Supports table and
//! JSON output modes.

use mediarr_core::HistoryDb;

use crate::output::OutputFormatter;
use crate::HistoryArgs;

/// Execute the history command.
pub fn execute(args: HistoryArgs) -> anyhow::Result<()> {
    let data_path = mediarr_core::config::default_data_path()?;
    let db = HistoryDb::open(&data_path)?;

    let batches = db.list_batches(args.limit)?;

    if batches.is_empty() {
        eprintln!("No rename history found");
        return Ok(());
    }

    let formatter = OutputFormatter::new(args.json);

    if args.json {
        formatter.history_json(&batches);
    } else {
        formatter.history_table(&batches);
    }

    Ok(())
}

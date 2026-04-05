//! Shared output formatting for CLI commands.
//!
//! Provides colorised table output, JSON mode, and verbose tree views.
//! Respects `NO_COLOR` environment variable and TTY detection per D-02.

use std::io::{self, IsTerminal, Write};

use comfy_table::{presets, Table};
use mediarr_core::{BatchSummary, RenameResult, ScanResult};
use owo_colors::OwoColorize;

/// Output formatter with color and JSON mode support.
pub struct OutputFormatter {
    /// Whether ANSI color codes are enabled.
    color_enabled: bool,
    /// Whether to output JSON instead of tables.
    _json_mode: bool,
}

impl OutputFormatter {
    /// Create a new formatter.
    ///
    /// Color is enabled when: not in JSON mode, stdout is a terminal,
    /// and the `NO_COLOR` environment variable is not set.
    pub fn new(json_mode: bool) -> Self {
        let color_enabled =
            !json_mode && io::stdout().is_terminal() && std::env::var("NO_COLOR").is_err();
        Self {
            color_enabled,
            _json_mode: json_mode,
        }
    }

    /// Display scan results as a table with STATUS, TYPE, TITLE, PROPOSED PATH columns.
    pub fn scan_table(&self, results: &[ScanResult]) {
        let mut table = Table::new();
        table.load_preset(presets::UTF8_FULL_CONDENSED);
        table.set_header(vec!["STATUS", "TYPE", "TITLE", "PROPOSED PATH"]);

        for r in results {
            let status = self.format_status(&r.status);
            let media_type = r.media_info.media_type.to_string();
            let title = &r.media_info.title;
            let proposed = r.proposed_path.display().to_string();

            table.add_row(vec![status, media_type, title.clone(), proposed]);
        }

        println!("{table}");
    }

    /// Display scan results as a verbose tree view with subtitle info.
    pub fn scan_verbose(&self, results: &[ScanResult]) {
        for r in results {
            let status = self.format_status(&r.status);
            println!(
                "{} {} [{}]",
                status,
                r.source_path.display(),
                r.media_info.media_type
            );
            println!("  -> {}", r.proposed_path.display());

            if !r.subtitles.is_empty() {
                println!("  Subtitles:");
                for sub in &r.subtitles {
                    let sub_type = sub
                        .subtitle_type
                        .as_ref()
                        .map(|t| format!(" ({t})"))
                        .unwrap_or_default();
                    println!(
                        "    [{}{}] {} -> {}",
                        sub.language,
                        sub_type,
                        sub.source_path.display(),
                        sub.proposed_path.display()
                    );
                }
            }

            if let Some(ref reason) = r.ambiguity_reason {
                println!("  Note: {reason}");
            }

            println!();
        }
    }

    /// Display scan results as JSON.
    pub fn scan_json(&self, results: &[ScanResult]) {
        let json = serde_json::to_string_pretty(results).expect("scan results should serialize");
        println!("{json}");
    }

    /// Display history batches as a table.
    pub fn history_table(&self, batches: &[BatchSummary]) {
        let mut table = Table::new();
        table.load_preset(presets::UTF8_FULL_CONDENSED);
        table.set_header(vec!["BATCH ID", "TIMESTAMP", "FILES", "TITLE"]);

        for b in batches {
            let short_id = if b.batch_id.len() > 8 {
                &b.batch_id[..8]
            } else {
                &b.batch_id
            };
            let title = b
                .entries
                .first()
                .map(|e| e.media_info.title.clone())
                .unwrap_or_else(|| "-".to_string());

            table.add_row(vec![
                short_id.to_string(),
                b.timestamp.clone(),
                b.file_count.to_string(),
                title,
            ]);
        }

        println!("{table}");
    }

    /// Display history batches as JSON.
    pub fn history_json(&self, batches: &[BatchSummary]) {
        let json = serde_json::to_string_pretty(batches).expect("batches should serialize");
        println!("{json}");
    }

    /// Display a numbered rename plan for confirmation.
    pub fn rename_plan(&self, results: &[(usize, &ScanResult)]) {
        println!("Rename plan:");
        println!();
        for (idx, r) in results {
            println!(
                "  {idx}. {} -> {}",
                r.source_path.display(),
                r.proposed_path.display()
            );
            for sub in &r.subtitles {
                println!(
                    "     [{}] {} -> {}",
                    sub.language,
                    sub.source_path.display(),
                    sub.proposed_path.display()
                );
            }
        }
        println!();
    }

    /// Display rename execution results.
    pub fn rename_results(&self, results: &[RenameResult]) {
        for r in results {
            if r.success {
                let icon = if self.color_enabled {
                    format!("{}", "ok".green())
                } else {
                    "ok".to_string()
                };
                println!(
                    "  {} {} -> {}",
                    icon,
                    r.source_path.display(),
                    r.dest_path.display()
                );
            } else {
                let icon = if self.color_enabled {
                    format!("{}", "FAIL".red())
                } else {
                    "FAIL".to_string()
                };
                let err = r.error.as_deref().unwrap_or("unknown error");
                println!(
                    "  {} {} -> {} ({})",
                    icon,
                    r.source_path.display(),
                    r.dest_path.display(),
                    err
                );
            }
        }
    }

    /// Display undo execution results.
    pub fn undo_results(&self, results: &[RenameResult]) {
        for r in results {
            if r.success {
                let icon = if self.color_enabled {
                    format!("{}", "ok".green())
                } else {
                    "ok".to_string()
                };
                println!(
                    "  {} {} -> {}",
                    icon,
                    r.source_path.display(),
                    r.dest_path.display()
                );
            } else {
                let icon = if self.color_enabled {
                    format!("{}", "FAIL".red())
                } else {
                    "FAIL".to_string()
                };
                let err = r.error.as_deref().unwrap_or("unknown error");
                println!("  {} {} ({})", icon, r.source_path.display(), err);
            }
        }
    }

    /// Format scan status with color.
    fn format_status(&self, status: &mediarr_core::ScanStatus) -> String {
        if self.color_enabled {
            match status {
                mediarr_core::ScanStatus::Ok => format!("{}", "ok".green()),
                mediarr_core::ScanStatus::Ambiguous => format!("{}", "!".yellow()),
                mediarr_core::ScanStatus::Conflict => format!("{}", "x".red()),
                mediarr_core::ScanStatus::Error => format!("{}", "x".red()),
            }
        } else {
            match status {
                mediarr_core::ScanStatus::Ok => "ok".to_string(),
                mediarr_core::ScanStatus::Ambiguous => "!".to_string(),
                mediarr_core::ScanStatus::Conflict => "x".to_string(),
                mediarr_core::ScanStatus::Error => "x".to_string(),
            }
        }
    }

    /// Print a summary line to stderr.
    pub fn print_summary(&self, msg: &str) {
        let _ = writeln!(io::stderr(), "{msg}");
    }
}

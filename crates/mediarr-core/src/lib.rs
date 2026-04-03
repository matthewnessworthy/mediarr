//! mediarr-core: Core library for media file renaming and organisation.
//!
//! This crate contains all business logic for Mediarr. It has zero knowledge
//! of any UI framework (Tauri, CLI). Both mediarr-cli and mediarr-tauri
//! depend on this crate.

pub mod config;
pub mod error;
pub mod fs_util;
pub mod history;
pub mod parser;
pub mod subtitle;
pub mod template;
pub mod types;

pub mod renamer;

// These modules will be added by subsequent plans:
// pub mod scanner;

pub use config::Config;
pub use error::{MediError, Result};
pub use fs_util::{path_to_utf8, safe_move};
pub use history::HistoryDb;
pub use parser::{parse_filename, parse_with_context};
pub use subtitle::SubtitleDiscovery;
pub use renamer::{RenamePlan, RenamePlanEntry, Renamer};
pub use template::TemplateEngine;
pub use types::*;

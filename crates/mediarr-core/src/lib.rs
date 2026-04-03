//! mediarr-core: Core library for media file renaming and organisation.
//!
//! This crate contains all business logic for Mediarr. It has zero knowledge
//! of any UI framework (Tauri, CLI). Both mediarr-cli and mediarr-tauri
//! depend on this crate.

pub mod error;
pub mod fs_util;
pub mod types;

pub mod history;

// These modules will be added by subsequent plans:
// pub mod config;
// pub mod parser;
// pub mod template;
// pub mod subtitle;
// pub mod scanner;
// pub mod renamer;

pub use error::{MediError, Result};
pub use fs_util::{path_to_utf8, safe_move};
pub use history::HistoryDb;
pub use types::*;

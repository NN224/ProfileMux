//! ProfileMux core library: browser discovery, profile models and safe operations.
//!
//! The TUI and CLI are both thin shells over this library; neither contains
//! browser-specific filesystem logic.

pub mod browsers;
pub mod cli;
pub mod doctor;
pub mod domain;
pub mod error;
pub mod fs;
pub mod platform;
pub mod tui;
pub mod update;

pub use error::{Error, Result};

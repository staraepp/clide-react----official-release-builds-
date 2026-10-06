//! The Tauri command surface.
//!
//! Commands stay thin on purpose: they translate arguments, call into a domain
//! module, and map errors to strings. Behaviour lives in the domain modules so
//! it can be tested without a webview.

pub mod dictation;
pub mod dictionary;
pub mod history;
pub mod models;
pub mod permissions;
pub mod providers;
pub mod settings;
pub mod updates;

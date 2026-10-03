//! Where the app's log goes.
//!
//! Launched from Finder, an app's stderr goes nowhere, which left every
//! "something went wrong with the microphone" report undiagnosable. Logs now
//! land in `~/Library/Logs/Clide/clide.log`, which Console.app shows and a user
//! can attach to a bug report. They carry no transcript text and no audio.
//!
//! Setting `CLIDE_LOG` keeps the old behaviour (verbose, to stderr) for
//! development.

use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;
use std::sync::Mutex;

use tracing_subscriber::EnvFilter;

/// The file is started afresh once it passes this, so it never grows without
/// bound. One previous run's worth of history is plenty for a bug report.
const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

pub fn init() {
    let from_environment = EnvFilter::try_from_env("CLIDE_LOG").ok();

    let Some(filter) = from_environment else {
        let filter = EnvFilter::new("clide=info,warn");
        match open_log_file() {
            Some(file) => tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_ansi(false)
                .with_writer(Mutex::new(file))
                .init(),
            None => tracing_subscriber::fmt().with_env_filter(filter).init(),
        }
        return;
    };

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

pub fn log_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Logs/Clide/clide.log"))
}

fn open_log_file() -> Option<File> {
    let path = log_path()?;
    fs::create_dir_all(path.parent()?).ok()?;

    let too_big = fs::metadata(&path).is_ok_and(|meta| meta.len() > MAX_LOG_BYTES);
    OpenOptions::new()
        .create(true)
        .append(!too_big)
        .write(true)
        .truncate(too_big)
        .open(path)
        .ok()
}

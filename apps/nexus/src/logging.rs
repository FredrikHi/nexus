//! Where log lines go: the console when there is one, and daily files when
//! there is a config file to put them beside.
//!
//! A service has no console, so without the files a failure would leave
//! nothing behind but "the service terminated unexpectedly" in Event Viewer.

use std::path::Path;

use anyhow::Context;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// How many daily files are kept before the oldest is deleted.
const KEEP_DAYS: usize = 14;

/// Must be held for as long as the process logs: dropping it flushes and stops
/// the background writer, and lines logged after that are lost.
pub struct Guard {
    _worker: Option<WorkerGuard>,
}

pub fn init(console: bool, log_dir: Option<&Path>) -> anyhow::Result<Guard> {
    let filter =
        || EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("nexus=info,warn"));

    let console_layer = console.then(|| tracing_subscriber::fmt::layer().with_filter(filter()));

    let (file_layer, guard) = match log_dir {
        Some(dir) => {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("cannot create the log folder {}", dir.display()))?;
            let appender = RollingFileAppender::builder()
                .rotation(Rotation::DAILY)
                .filename_prefix("nexus")
                .filename_suffix("log")
                .max_log_files(KEEP_DAYS)
                .build(dir)
                .with_context(|| format!("cannot open a log file in {}", dir.display()))?;
            // Written on a background thread, so a slow disk never holds up a
            // request that happens to log.
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let layer = tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_ansi(false)
                .with_filter(filter());
            (Some(layer), Some(guard))
        }
        None => (None, None),
    };

    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();

    Ok(Guard { _worker: guard })
}

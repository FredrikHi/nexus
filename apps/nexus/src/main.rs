//! `nexus`: runs Nexus on Windows without Docker.
//!
//! PostgreSQL (unless DATABASE_URL points elsewhere), the API, the auth
//! service and the web app in front of them, as one Windows service or in a
//! console:
//!
//! ```text
//! nexus install --url https://nexus.example.com   # as a service
//! nexus run                                       # in this console
//! ```

mod app;
mod cli;
mod config;
mod config_file;
mod front_door;
mod logging;
mod postgres;
mod services;
mod supervisor;
#[cfg(windows)]
mod win;

use clap::Parser;

use cli::{Cli, Command};
use config::Config;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Run { config: None }) {
        Command::Run { config } => {
            // With a config file, logs go beside it as the service's do; without
            // one this is a development run, and the console is enough.
            let log_dir = config
                .as_deref()
                .and_then(|file| file.parent())
                .map(|dir| dir.join("logs"));
            let _logging = logging::init(true, log_dir.as_deref())?;
            let config = Config::load(config.as_deref())?;
            runtime()?.block_on(app::run(config, ctrl_c()))
        }

        #[cfg(windows)]
        Command::Install(args) => runtime()?.block_on(win::install::install(args)),
        #[cfg(windows)]
        Command::Uninstall => win::install::uninstall(),
        #[cfg(windows)]
        Command::Service { config } => win::service::run(config),

        #[cfg(not(windows))]
        Command::Install(_) | Command::Uninstall | Command::Service { .. } => {
            anyhow::bail!(
                "installing as a service is for Windows; elsewhere, use the Docker images"
            )
        }
    }
}

fn runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?)
}

/// Resolves on Ctrl+C.
async fn ctrl_c() {
    if let Err(err) = tokio::signal::ctrl_c().await {
        // Without a signal handler there is no clean way to stop; say so and
        // keep running rather than exiting on the spot.
        tracing::error!(error = %err, "could not listen for Ctrl+C");
        std::future::pending::<()>().await;
    }
}

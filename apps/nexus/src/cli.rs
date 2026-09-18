//! The command line.

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "nexus",
    version,
    about = "Runs Nexus on Windows without Docker",
    long_about = "Runs Nexus on Windows without Docker: PostgreSQL, the API, the auth \
                  service and the web app, as one Windows service.\n\n\
                  Install it with `nexus install --url https://nexus.example.com`."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run in this console until Ctrl+C. Reads the environment, or a config file.
    Run {
        /// A nexus.env to read instead of the environment.
        #[arg(long)]
        config: Option<PathBuf>,
    },

    /// Install and start the Windows service. Needs an elevated console.
    Install(InstallArgs),

    /// Stop and remove the Windows service. The data and the config file stay.
    Uninstall,

    /// What Windows starts. Not for running by hand.
    #[command(hide = true)]
    Service {
        #[arg(long)]
        config: PathBuf,
    },
}

#[derive(Debug, clap::Args)]
pub struct InstallArgs {
    /// The address people type in the browser, e.g. https://nexus.example.com.
    /// Required the first time; a reinstall keeps the previous one.
    #[arg(long)]
    pub url: Option<String>,

    /// Where the front door listens. Default: 127.0.0.1:5173, reachable from
    /// this machine only, for IIS or another reverse proxy to forward to.
    #[arg(long)]
    pub listen: Option<SocketAddr>,

    /// Node.js to run the auth service with. Default: node.exe found on PATH.
    #[arg(long)]
    pub node: Option<PathBuf>,

    /// Use this PostgreSQL server instead of the one Nexus runs itself.
    #[arg(long)]
    pub database_url: Option<String>,

    /// Where to keep the config file; data and logs go beside it.
    /// Default: %ProgramData%\Nexus\nexus.env.
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Install without starting.
    #[arg(long)]
    pub no_start: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_command_means_run_from_the_environment() {
        let cli = Cli::try_parse_from(["nexus"]).unwrap();
        assert!(cli.command.is_none());
    }

    #[test]
    fn install_takes_the_url_and_optional_overrides() {
        let cli = Cli::try_parse_from([
            "nexus",
            "install",
            "--url",
            "https://nexus.brandkontoret.se",
            "--listen",
            "127.0.0.1:8088",
            "--node",
            r"C:\Program Files\nodejs\node.exe",
        ])
        .unwrap();

        let Some(Command::Install(args)) = cli.command else {
            panic!("expected install");
        };
        assert_eq!(args.url.as_deref(), Some("https://nexus.brandkontoret.se"));
        assert_eq!(args.listen, Some("127.0.0.1:8088".parse().unwrap()));
        assert!(!args.no_start);
    }

    #[test]
    fn a_listen_address_that_is_not_one_is_refused_before_anything_happens() {
        assert!(Cli::try_parse_from(["nexus", "install", "--listen", "localhost"]).is_err());
    }

    #[test]
    fn the_service_entry_point_needs_its_config() {
        assert!(Cli::try_parse_from(["nexus", "service"]).is_err());
        let cli = Cli::try_parse_from([
            "nexus",
            "service",
            "--config",
            r"C:\ProgramData\Nexus\nexus.env",
        ])
        .unwrap();
        assert!(matches!(cli.command, Some(Command::Service { .. })));
    }
}

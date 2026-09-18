//! Nexus's own PostgreSQL, for when nobody set DATABASE_URL.
//!
//! PostgreSQL runs in the foreground as a supervised child like the other two
//! services, not as a Windows service of its own. It starts and stops with
//! Nexus, restarts if it crashes, and there is one thing to install.
//!
//! Its data lives in `<DATA_DIR>/postgres`. The superuser password is generated
//! on first start and kept beside it in `<DATA_DIR>/postgres.password`: the API
//! and the auth service need it on every start, and nobody should ever have to
//! type it.

#[cfg(test)]
mod tests;

use std::fmt;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context};
use rand::rngs::SysRng;
use rand::TryRng;

use crate::config::{exe, EmbeddedPostgres};
use crate::supervisor::{ChildSpec, CommandSpec, GracefulStop, Health, LineFormat, RestartPolicy};

/// The name docker-compose.yml gives the database, so a dump from one
/// deployment restores into the other without renaming anything.
pub const DATABASE: &str = "integration_observability";
const SUPERUSER: &str = "postgres";

/// Makes the PostgreSQL tools print English whatever the Windows display
/// language. Their localized output comes in the console's code page, which
/// arrives in our UTF-8 log as mojibake, and an English error is also the one
/// a search engine finds.
const ENGLISH: (&str, &str) = ("LC_MESSAGES", "C");

/// The superuser password. A type of its own so it cannot end up in a log
/// line by way of `{:?}`.
#[derive(Clone)]
pub struct Password(String);

impl Password {
    fn generate() -> anyhow::Result<Self> {
        let mut bytes = [0u8; 32];
        // The operating system's generator, as the API uses for its tokens.
        // Hex, so the password can sit inside a postgres:// URL unescaped.
        SysRng
            .try_fill_bytes(&mut bytes)
            .context("the operating system's random number generator is unavailable")?;
        Ok(Self(hex::encode(bytes)))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Password(<redacted>)")
    }
}

/// PostgreSQL's reason for refusing to start, plus what to do about the one
/// refusal people hit without having done anything wrong.
///
/// PostgreSQL will not run with administrator rights: a database server that
/// can be talked into running code would otherwise hand out the whole machine.
/// On Windows Server that catches anyone logged in as the built-in
/// Administrator, whose every console is elevated.
fn explain_refusal(pgdata: &Path, stderr: &str) -> String {
    let reason = stderr.trim();
    if reason.contains("administrative permissions") {
        format!(
            "PostgreSQL refuses to run with administrator rights, and this console is elevated. \
             Start nexus from a console that is not \"Run as administrator\". \
             Installed as a Windows service it runs under its own unprivileged account, \
             where this does not apply. ({reason})"
        )
    } else {
        format!("PostgreSQL cannot use {}: {reason}", pgdata.display())
    }
}

pub struct Embedded {
    bin: PathBuf,
    data_dir: PathBuf,
    pgdata: PathBuf,
    password_file: PathBuf,
    addr: SocketAddr,
}

impl Embedded {
    pub fn new(settings: &EmbeddedPostgres, data_dir: &Path) -> Self {
        Self {
            bin: settings.pg_dir.join("bin"),
            data_dir: data_dir.to_path_buf(),
            pgdata: data_dir.join("postgres"),
            password_file: data_dir.join("postgres.password"),
            addr: settings.addr,
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn pgdata(&self) -> &Path {
        &self.pgdata
    }

    /// Creates the database cluster on first start and returns its password,
    /// or finds the existing one and reads its password.
    ///
    /// Ends by asking the server binary to read the cluster's configuration,
    /// which fails the same way starting it would. That turns "PostgreSQL
    /// refuses to run as an administrator" into one clear error here, rather
    /// than a crash repeated by the supervisor every minute.
    pub async fn prepare(&self) -> anyhow::Result<Password> {
        tokio::fs::create_dir_all(&self.data_dir)
            .await
            .with_context(|| format!("cannot create DATA_DIR {}", self.data_dir.display()))?;

        let password = if self.pgdata.join("PG_VERSION").is_file() {
            self.read_password().await?
        } else {
            self.create_cluster().await?
        };

        let version = self
            .tool("postgres")
            .args(["-D", &self.pgdata_arg(), "-C", "server_version"])
            .output()
            .await
            .context("could not run postgres")?;
        if !version.status.success() {
            bail!(explain_refusal(
                &self.pgdata,
                &String::from_utf8_lossy(&version.stderr)
            ));
        }
        tracing::info!(
            version = %String::from_utf8_lossy(&version.stdout).trim(),
            data = %self.pgdata.display(),
            "PostgreSQL ready to start"
        );

        Ok(password)
    }

    async fn read_password(&self) -> anyhow::Result<Password> {
        let password = tokio::fs::read_to_string(&self.password_file)
            .await
            .with_context(|| {
                format!(
                    "the database in {} exists but its password file {} cannot be read. \
                     Restore that file from a backup of DATA_DIR",
                    self.pgdata.display(),
                    self.password_file.display()
                )
            })?;
        let password = password.trim();
        if password.is_empty() {
            bail!("{} is empty", self.password_file.display());
        }
        Ok(Password(password.to_string()))
    }

    async fn create_cluster(&self) -> anyhow::Result<Password> {
        // Never clear out a folder that has something in it: it may be a
        // database whose PG_VERSION was lost, and deleting it would be the one
        // unrecoverable mistake this code could make.
        if let Ok(mut entries) = tokio::fs::read_dir(&self.pgdata).await {
            if entries.next_entry().await?.is_some() {
                bail!(
                    "{} is not empty but does not look like a PostgreSQL data folder. \
                     Move it somewhere else and start Nexus again to create a new database",
                    self.pgdata.display()
                );
            }
        }

        tracing::info!(data = %self.pgdata.display(), "creating the database, first start only");
        let password = Password::generate()?;
        // Written before initdb and read by it, so the password on disk is by
        // construction the one the cluster was created with.
        tokio::fs::write(&self.password_file, password.expose())
            .await
            .with_context(|| format!("cannot write {}", self.password_file.display()))?;

        let output = self
            .tool("initdb")
            .args([
                "-D",
                &self.pgdata_arg(),
                "-U",
                SUPERUSER,
                &format!("--pwfile={}", self.password_file.display()),
                // A password for every connection, local ones included. Other
                // accounts on the same server can reach loopback too.
                "--auth=scram-sha-256",
                "--encoding=UTF8",
                // PostgreSQL's own locale rather than Windows': independent of
                // the server's display language, and immune to the collation
                // changes an OS update can bring, which silently corrupt indexes.
                "--locale=C",
                "--locale-provider=builtin",
                "--builtin-locale=C.UTF-8",
                "--no-instructions",
            ])
            .output()
            .await
            .context("could not run initdb")?;
        if !output.status.success() {
            bail!(
                "initdb failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(password)
    }

    /// The server itself, in the foreground so the supervisor sees it exit.
    pub fn child_spec(&self) -> ChildSpec {
        let host = self.addr.ip().to_string();
        let port = self.addr.port().to_string();

        ChildSpec {
            name: "postgres".to_string(),
            run: self.command(
                "postgres",
                &[
                    "-D",
                    &self.pgdata_arg(),
                    "-p",
                    &port,
                    // Settings on the command line, not in postgresql.conf, so
                    // they are Nexus's to decide and cannot drift from what the
                    // other two services are told.
                    "-c",
                    &format!("listen_addresses={host}"),
                    // TCP only. A socket file is one more way in to secure.
                    "-c",
                    "unix_socket_directories=",
                ],
            ),
            before_start: None,
            // Does not log in, so it needs no password; it only asks whether
            // the server is accepting connections yet.
            health: Some(Health::Command(
                self.command("pg_isready", &["-q", "-h", &host, "-p", &port, "-t", "2"]),
            )),
            // A fast shutdown: open transactions are rolled back, a checkpoint
            // is written, and the next start needs no crash recovery.
            graceful_stop: Some(GracefulStop {
                command: self.command(
                    "pg_ctl",
                    &[
                        "stop",
                        "-D",
                        &self.pgdata_arg(),
                        "-m",
                        "fast",
                        "-w",
                        "-t",
                        "60",
                    ],
                ),
                timeout: Duration::from_secs(90),
            }),
            restart: RestartPolicy::default(),
            output: LineFormat::Postgres,
        }
    }

    /// What the API and the auth service connect with.
    pub fn database_url(&self, password: &Password) -> String {
        format!(
            "postgres://{SUPERUSER}:{}@{}/{DATABASE}",
            password.expose(),
            self.addr
        )
    }

    /// Creates Nexus's database if it is not there yet. initdb only creates
    /// `postgres`, and the API's migrations expect theirs to exist.
    pub async fn ensure_database(&self, password: &Password) -> anyhow::Result<()> {
        let exists = self
            .psql(
                password,
                &format!("SELECT 1 FROM pg_database WHERE datname = '{DATABASE}'"),
            )
            .await?;
        if exists.trim() == "1" {
            return Ok(());
        }
        self.psql(password, &format!("CREATE DATABASE {DATABASE}"))
            .await?;
        tracing::info!(database = DATABASE, "created the database");
        Ok(())
    }

    async fn psql(&self, password: &Password, sql: &str) -> anyhow::Result<String> {
        let host = self.addr.ip().to_string();
        let port = self.addr.port().to_string();
        let output = self
            .tool("psql")
            .args([
                "-h",
                &host,
                "-p",
                &port,
                "-U",
                SUPERUSER,
                "-d",
                "postgres",
                // No .psqlrc, unaligned tuples only, stop at the first error.
                "-X",
                "-A",
                "-t",
                "-v",
                "ON_ERROR_STOP=1",
                "-c",
                sql,
            ])
            // The environment, not the command line: arguments are visible to
            // every user on the machine in the process list.
            .env("PGPASSWORD", password.expose())
            .output()
            .await
            .context("could not run psql")?;
        if !output.status.success() {
            bail!(
                "psql failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// A one-off PostgreSQL tool, with output captured.
    fn tool(&self, name: &str) -> tokio::process::Command {
        let mut command = tokio::process::Command::new(self.bin.join(exe(name)));
        command
            .current_dir(&self.data_dir)
            .env(ENGLISH.0, ENGLISH.1)
            .kill_on_drop(true);
        command
    }

    /// A PostgreSQL tool described for the supervisor.
    fn command(&self, name: &str, args: &[&str]) -> CommandSpec {
        CommandSpec {
            program: self.bin.join(exe(name)),
            args: args.iter().map(|a| a.to_string()).collect(),
            cwd: self.data_dir.clone(),
            env: vec![(ENGLISH.0.to_string(), ENGLISH.1.to_string())],
        }
    }

    fn pgdata_arg(&self) -> String {
        self.pgdata.display().to_string()
    }
}

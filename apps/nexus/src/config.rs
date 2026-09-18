//! Configuration, read once at startup.
//!
//! Everything a mistake could break is checked here, before anything starts.
//! A wrong path found at startup is one clear line; found by a child process
//! it is the same crash repeated every minute in the log.

use std::fmt;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};

use crate::config_file;

#[derive(Clone)]
pub struct Config {
    /// Where the front door listens.
    pub listen: SocketAddr,
    /// The built web app: the folder holding `index.html` and `assets/`.
    pub web_dir: PathBuf,

    /// Where the API listens. Loopback only; the front door is the way in.
    pub api_addr: SocketAddr,
    /// Where the auth service listens. Loopback only, same reason.
    pub auth_addr: SocketAddr,

    /// The API executable.
    pub api_exe: PathBuf,
    /// The bundled auth service: `index.cjs` and `migrate.cjs`, each one file
    /// with its dependencies inside, so no `node_modules` and no npm.
    pub auth_dir: PathBuf,
    /// Node.js. A bare `node` is looked up on PATH.
    pub node_exe: PathBuf,

    /// Where Nexus keeps what it creates: the database files and the password
    /// that opens them.
    pub data_dir: PathBuf,
    pub database: Database,

    /// The public origin people type. Sign-in is refused from any other.
    pub app_url: String,
    /// Signs sessions and encrypts the auth service's signing keys at rest.
    pub auth_secret: String,
    /// The API's log filter. Not RUST_LOG, which is this process's own and
    /// would otherwise be inherited by the API and filter out all of its logs.
    pub api_log_filter: String,
}

/// Which Postgres the API and the auth service use.
#[derive(Clone)]
pub enum Database {
    /// DATABASE_URL was set: a server someone already runs.
    External { url: String },
    /// DATABASE_URL was not set: Nexus runs its own.
    Embedded(EmbeddedPostgres),
}

#[derive(Debug, Clone)]
pub struct EmbeddedPostgres {
    /// PostgreSQL's own folder, holding `bin/`, `lib/` and `share/`.
    pub pg_dir: PathBuf,
    /// Where it listens. Loopback only.
    pub addr: SocketAddr,
}

/// Written by hand rather than derived, so a `{:?}` in a log line or a failed
/// test can never print the secret or the database password.
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("listen", &self.listen)
            .field("web_dir", &self.web_dir)
            .field("api_addr", &self.api_addr)
            .field("auth_addr", &self.auth_addr)
            .field("api_exe", &self.api_exe)
            .field("auth_dir", &self.auth_dir)
            .field("node_exe", &self.node_exe)
            .field("data_dir", &self.data_dir)
            .field("database", &self.database)
            .field("app_url", &self.app_url)
            .field("auth_secret", &"<redacted>")
            .field("api_log_filter", &self.api_log_filter)
            .finish()
    }
}

impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Database::External { .. } => f.write_str("External { url: <redacted> }"),
            Database::Embedded(embedded) => f.debug_tuple("Embedded").field(embedded).finish(),
        }
    }
}

impl Config {
    /// Reads the configuration from `config_file` if one is given, and from the
    /// environment otherwise. Never a mix of the two.
    ///
    /// With a file, the environment is ignored entirely. A service runs with the
    /// machine's environment, and a server that hosts other applications may
    /// well have a DATABASE_URL or an APP_URL set for one of them. Mixing would
    /// let that quietly point Nexus at someone else's database.
    pub fn load(config_file: Option<&Path>) -> anyhow::Result<Self> {
        let exe_dir = exe_dir()?;
        // An empty value counts as unset, the same rule the auth service
        // applies, so `AUTH_SECRET=` does not read as a secret.
        match config_file {
            Some(path) => {
                let settings = config_file::read(path)?;
                let config_dir = path
                    .parent()
                    .context("the config file has no parent directory")?;
                Self::from_lookup(&exe_dir, Some(config_dir), |key| {
                    settings.get(key).filter(|v| !v.is_empty()).cloned()
                })
            }
            None => Self::from_lookup(&exe_dir, None, |key| {
                std::env::var(key).ok().filter(|v| !v.is_empty())
            }),
        }
        .with_context(|| match config_file {
            Some(path) => format!("in {}", path.display()),
            None => "in the environment".to_string(),
        })
    }

    /// Reads settings through `lookup` rather than straight from the process
    /// environment, so tests can hand in a map instead of mutating global
    /// state other tests running in parallel can see.
    ///
    /// `exe_dir` is where the release layout is looked for when a path is not
    /// given. `config_dir` is where the config file lives, if there is one: data
    /// is kept beside it unless DATA_DIR says otherwise.
    pub fn from_lookup(
        exe_dir: &Path,
        config_dir: Option<&Path>,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> anyhow::Result<Self> {
        let path_or =
            |key: &str, default: PathBuf| lookup(key).map(PathBuf::from).unwrap_or(default);

        // Loopback by default. Anyone who can reach Nexus can create an
        // account, so nothing is reachable until someone decides it should be.
        // 5173 is the port APP_URL defaults to everywhere.
        let listen = address(&lookup, "LISTEN_ADDR", "127.0.0.1:5173")?;
        // Each service's usual port plus 10000: close enough to recognise, far
        // enough from 8080, 3000-something and 5432 that a Windows server
        // running other software is unlikely to have them taken.
        let api_addr = loopback("API_ADDR", address(&lookup, "API_ADDR", "127.0.0.1:18080")?)?;
        let auth_addr = loopback(
            "AUTH_ADDR",
            address(&lookup, "AUTH_ADDR", "127.0.0.1:13010")?,
        )?;

        let web_dir = path_or("WEB_DIR", exe_dir.join("web"));
        require_file(
            "WEB_DIR",
            &web_dir.join("index.html"),
            "the built web app: run npm run build in apps/web and use its dist folder",
        )?;

        let api_exe = path_or("API_EXE", exe_dir.join("bin").join(exe("integration-api")));
        require_file(
            "API_EXE",
            &api_exe,
            "the API executable: run cargo build --release in apps/api",
        )?;

        let auth_dir = path_or("AUTH_DIR", exe_dir.join("auth"));
        for bundle in ["index.cjs", "migrate.cjs"] {
            require_file(
                "AUTH_DIR",
                &auth_dir.join(bundle),
                "the bundled auth service: run npm run bundle in apps/auth and use its dist folder",
            )?;
        }

        let node_exe = path_or("NODE_EXE", PathBuf::from("node"));

        // Beside the config file when there is one. Otherwise %ProgramData% on
        // Windows: the machine-wide place for a service's data, which survives
        // an upgrade that replaces the program folder.
        let data_dir = path_or(
            "DATA_DIR",
            config_dir
                .map(Path::to_path_buf)
                .or_else(|| lookup("ProgramData").map(|p| PathBuf::from(p).join("Nexus")))
                .unwrap_or_else(|| exe_dir.join("data")),
        );

        let database = match lookup("DATABASE_URL") {
            Some(url) => Database::External { url },
            None => {
                let pg_dir = path_or("PG_DIR", exe_dir.join("pgsql"));
                for tool in ["postgres", "initdb", "pg_ctl", "pg_isready", "psql"] {
                    require_file(
                        "PG_DIR",
                        &pg_dir.join("bin").join(exe(tool)),
                        "a PostgreSQL 18 folder holding bin/, lib/ and share/, or set DATABASE_URL to use a server you already run",
                    )?;
                }
                let addr = loopback(
                    "POSTGRES_ADDR",
                    address(&lookup, "POSTGRES_ADDR", "127.0.0.1:15432")?,
                )?;
                Database::Embedded(EmbeddedPostgres { pg_dir, addr })
            }
        };

        let mut ports = vec![
            ("LISTEN_ADDR", listen),
            ("API_ADDR", api_addr),
            ("AUTH_ADDR", auth_addr),
        ];
        if let Database::Embedded(embedded) = &database {
            ports.push(("POSTGRES_ADDR", embedded.addr));
        }
        distinct_ports(&ports)?;

        let auth_secret = lookup("AUTH_SECRET")
            .context("AUTH_SECRET must be set. Generate one with: openssl rand -hex 32")?;
        let app_url = lookup("APP_URL")
            .unwrap_or_else(|| "http://localhost:5173".to_string())
            .trim_end_matches('/')
            .to_string();
        let api_log_filter =
            lookup("API_RUST_LOG").unwrap_or_else(|| "integration_api=info,warn".to_string());

        Ok(Self {
            listen,
            web_dir,
            api_addr,
            auth_addr,
            api_exe,
            auth_dir,
            node_exe,
            data_dir,
            database,
            app_url,
            auth_secret,
            api_log_filter,
        })
    }
}

/// The folder nexus.exe runs from, where a release keeps web/, bin/, auth/
/// and pgsql/.
pub fn exe_dir() -> anyhow::Result<PathBuf> {
    Ok(std::env::current_exe()
        .context("cannot locate the running executable")?
        .parent()
        .context("the running executable has no parent directory")?
        .to_path_buf())
}

/// Where `nexus install` puts the config file unless told otherwise.
pub fn default_config_file() -> anyhow::Result<PathBuf> {
    let program_data = std::env::var_os("ProgramData")
        .context("%ProgramData% is not set; pass --config with a path")?;
    Ok(PathBuf::from(program_data).join("Nexus").join("nexus.env"))
}

/// `postgres` -> `postgres.exe` on Windows, unchanged elsewhere.
pub fn exe(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

fn address(
    lookup: &impl Fn(&str) -> Option<String>,
    key: &str,
    default: &str,
) -> anyhow::Result<SocketAddr> {
    let value = lookup(key).unwrap_or_else(|| default.to_string());
    value
        .parse()
        .with_context(|| format!("{key} must be an ip:port like {default}, got {value:?}"))
}

/// The services behind the front door must not be reachable on their own. If
/// they were, the auth service would take sign-ups from the network without
/// going through whatever TLS and access rules sit in front of Nexus, and the
/// database would be one guessed password away.
fn loopback(key: &str, addr: SocketAddr) -> anyhow::Result<SocketAddr> {
    if !addr.ip().is_loopback() {
        bail!("{key} must be a loopback address like 127.0.0.1:port; {addr} would expose the service directly");
    }
    Ok(addr)
}

/// Compared on port alone: 0.0.0.0:5173 and 127.0.0.1:5173 collide too.
fn distinct_ports(addresses: &[(&str, SocketAddr)]) -> anyhow::Result<()> {
    for (i, (key_a, a)) in addresses.iter().enumerate() {
        for (key_b, b) in &addresses[i + 1..] {
            if a.port() == b.port() {
                bail!("{key_a} and {key_b} both use port {}", a.port());
            }
        }
    }
    Ok(())
}

fn require_file(key: &str, path: &Path, expected: &str) -> anyhow::Result<()> {
    if !path.is_file() {
        bail!("{} does not exist. Set {key} to {expected}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tempfile::TempDir;

    use super::*;

    /// The release layout: web/, bin/, auth/ and pgsql/ next to nexus.exe.
    fn release_layout() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let touch = |relative: &[&str]| {
            let path = relative
                .iter()
                .fold(root.to_path_buf(), |p, part| p.join(part));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        };
        touch(&["web", "index.html"]);
        touch(&["bin", &exe("integration-api")]);
        touch(&["auth", "index.cjs"]);
        touch(&["auth", "migrate.cjs"]);
        for tool in ["postgres", "initdb", "pg_ctl", "pg_isready", "psql"] {
            touch(&["pgsql", "bin", &exe(tool)]);
        }
        dir
    }

    fn required() -> HashMap<&'static str, String> {
        HashMap::from([("AUTH_SECRET", "0123456789abcdef".to_string())])
    }

    fn load(dir: &Path, vars: HashMap<&'static str, String>) -> anyhow::Result<Config> {
        Config::from_lookup(dir, None, |key| vars.get(key).cloned())
    }

    #[test]
    fn release_layout_needs_only_the_auth_secret() {
        let dir = release_layout();
        let config = load(dir.path(), required()).unwrap();

        assert_eq!(config.listen, "127.0.0.1:5173".parse().unwrap());
        assert_eq!(config.api_addr, "127.0.0.1:18080".parse().unwrap());
        assert_eq!(config.auth_addr, "127.0.0.1:13010".parse().unwrap());
        assert_eq!(config.web_dir, dir.path().join("web"));
        assert_eq!(config.auth_dir, dir.path().join("auth"));
        assert_eq!(config.node_exe, PathBuf::from("node"));
        assert_eq!(config.app_url, "http://localhost:5173");
        let Database::Embedded(embedded) = &config.database else {
            panic!("expected the embedded database, got {:?}", config.database);
        };
        assert_eq!(embedded.pg_dir, dir.path().join("pgsql"));
        assert_eq!(embedded.addr, "127.0.0.1:15432".parse().unwrap());
    }

    #[test]
    fn data_lives_under_program_data_on_windows() {
        let dir = release_layout();
        let mut vars = required();
        vars.insert("ProgramData", r"C:\ProgramData".to_string());

        let config = load(dir.path(), vars).unwrap();

        assert_eq!(
            config.data_dir,
            PathBuf::from(r"C:\ProgramData").join("Nexus")
        );
    }

    #[test]
    fn data_lives_beside_the_config_file_when_there_is_one() {
        let dir = release_layout();
        let vars = required();
        let config_dir = dir.path().join("config");

        let config =
            Config::from_lookup(dir.path(), Some(&config_dir), |key| vars.get(key).cloned())
                .unwrap();

        assert_eq!(config.data_dir, config_dir);
    }

    #[test]
    fn with_a_config_file_the_environment_is_not_consulted() {
        let dir = release_layout();
        let file = dir.path().join("nexus.env");
        crate::config_file::write(
            &file,
            &crate::config_file::Settings::from([
                ("AUTH_SECRET".to_string(), "from-the-file".to_string()),
                (
                    "PG_DIR".to_string(),
                    dir.path().join("pgsql").display().to_string(),
                ),
                (
                    "WEB_DIR".to_string(),
                    dir.path().join("web").display().to_string(),
                ),
                (
                    "API_EXE".to_string(),
                    dir.path()
                        .join("bin")
                        .join(exe("integration-api"))
                        .display()
                        .to_string(),
                ),
                (
                    "AUTH_DIR".to_string(),
                    dir.path().join("auth").display().to_string(),
                ),
            ]),
        )
        .unwrap();

        // This test process's own environment is whatever cargo gave it, and may
        // hold a DATABASE_URL. Loading from the file must not pick that up.
        let config = Config::load(Some(&file)).unwrap();

        assert_eq!(config.auth_secret, "from-the-file");
        assert!(matches!(config.database, Database::Embedded(_)));
        assert_eq!(config.data_dir, dir.path());
    }

    #[test]
    fn data_dir_falls_back_next_to_the_executable() {
        let dir = release_layout();
        let config = load(dir.path(), required()).unwrap();
        assert_eq!(config.data_dir, dir.path().join("data"));
    }

    #[test]
    fn database_url_means_no_embedded_postgres_is_needed() {
        let dir = release_layout();
        std::fs::remove_dir_all(dir.path().join("pgsql")).unwrap();
        let mut vars = required();
        vars.insert(
            "DATABASE_URL",
            "postgres://u:p@db.internal/nexus".to_string(),
        );

        let config = load(dir.path(), vars).unwrap();

        assert!(matches!(config.database, Database::External { .. }));
    }

    #[test]
    fn missing_postgres_binaries_name_both_ways_out() {
        let dir = release_layout();
        std::fs::remove_file(dir.path().join("pgsql").join("bin").join(exe("pg_ctl"))).unwrap();

        let err = load(dir.path(), required()).unwrap_err().to_string();

        assert!(
            err.contains("PG_DIR") && err.contains("DATABASE_URL"),
            "{err}"
        );
    }

    #[test]
    fn debug_output_never_shows_secrets() {
        let dir = release_layout();
        let mut vars = required();
        vars.insert("DATABASE_URL", "postgres://u:hunter2@db/nexus".to_string());

        let printed = format!("{:?}", load(dir.path(), vars).unwrap());

        assert!(!printed.contains("hunter2"), "{printed}");
        assert!(!printed.contains("0123456789abcdef"), "{printed}");
    }

    #[test]
    fn app_url_loses_its_trailing_slash() {
        let dir = release_layout();
        let mut vars = required();
        vars.insert("APP_URL", "https://nexus.brandkontoret.se/".to_string());

        let config = load(dir.path(), vars).unwrap();

        assert_eq!(config.app_url, "https://nexus.brandkontoret.se");
    }

    #[test]
    fn missing_auth_secret_says_how_to_make_one() {
        let dir = release_layout();
        let err = load(dir.path(), HashMap::new()).unwrap_err().to_string();
        assert!(err.contains("openssl rand -hex 32"), "{err}");
    }

    #[test]
    fn backend_on_a_public_address_is_refused() {
        let dir = release_layout();
        let mut vars = required();
        vars.insert("POSTGRES_ADDR", "0.0.0.0:15432".to_string());

        let err = load(dir.path(), vars).unwrap_err().to_string();

        assert!(
            err.contains("POSTGRES_ADDR must be a loopback address"),
            "{err}"
        );
    }

    #[test]
    fn two_services_on_one_port_are_refused() {
        let dir = release_layout();
        let mut vars = required();
        vars.insert("API_ADDR", "127.0.0.1:15432".to_string());

        let err = load(dir.path(), vars).unwrap_err().to_string();

        assert!(
            err.contains("API_ADDR and POSTGRES_ADDR both use port 15432"),
            "{err}"
        );
    }

    #[test]
    fn missing_web_build_names_the_setting_and_the_fix() {
        let dir = release_layout();
        std::fs::remove_file(dir.path().join("web").join("index.html")).unwrap();

        let err = load(dir.path(), required()).unwrap_err().to_string();

        assert!(
            err.contains("WEB_DIR") && err.contains("npm run build"),
            "{err}"
        );
    }

    #[test]
    fn missing_api_executable_names_the_setting() {
        let dir = release_layout();
        let mut vars = required();
        vars.insert("API_EXE", dir.path().join("nope.exe").display().to_string());

        let err = load(dir.path(), vars).unwrap_err().to_string();

        assert!(err.contains("API_EXE"), "{err}");
    }
}

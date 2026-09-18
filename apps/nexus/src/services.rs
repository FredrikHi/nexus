//! What the API and the auth service are, in terms the supervisor runs.
//!
//! The environment each gets is what docker-compose.yml gave the matching
//! container, with addresses pointing at loopback instead of service names.

use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context};

use crate::config::Config;
use crate::supervisor::{ChildSpec, CommandSpec, Health, LineFormat, RestartPolicy};

/// The oldest Node the auth service is built and tested against.
const MIN_NODE_MAJOR: u32 = 22;

/// `database_url` is passed rather than read from `config`: with the embedded
/// Postgres it is only known once that has been prepared.
pub fn api(config: &Config, database_url: &str) -> ChildSpec {
    let env = vec![
        pair("HOST", config.api_addr.ip().to_string()),
        pair("PORT", config.api_addr.port().to_string()),
        pair("DATABASE_URL", database_url),
        pair("APP_URL", &config.app_url),
        // Straight to the auth service over loopback, not through the front
        // door, so token validation does not depend on the proxy.
        pair(
            "AUTH_JWKS_URL",
            format!("http://{}/api/auth/jwks", config.auth_addr),
        ),
        pair("RUST_LOG", &config.api_log_filter),
        // Its lines end up inside ours; colour codes would be noise there.
        pair("NO_COLOR", "1"),
    ];

    ChildSpec {
        name: "api".to_string(),
        run: CommandSpec {
            program: config.api_exe.clone(),
            args: Vec::new(),
            // Its own folder, not wherever nexus was started from. The API loads
            // a .env from its working directory, and a stray one there would
            // quietly add settings nobody meant it to have.
            cwd: parent_dir(&config.api_exe),
            env,
        },
        before_start: None,
        health: Some(Health::Http(format!(
            "http://{}/api/v1/health",
            config.api_addr
        ))),
        graceful_stop: None,
        restart: RestartPolicy::default(),
        output: LineFormat::Tracing,
    }
}

pub fn auth(config: &Config, database_url: &str) -> ChildSpec {
    let env = vec![
        pair("HOST", config.auth_addr.ip().to_string()),
        pair("PORT", config.auth_addr.port().to_string()),
        pair("DATABASE_URL", database_url),
        pair("APP_URL", &config.app_url),
        pair("AUTH_SECRET", &config.auth_secret),
        pair("NODE_ENV", "production"),
        pair("NO_COLOR", "1"),
        pair("FORCE_COLOR", "0"),
    ];

    let node = |bundle: &str| CommandSpec {
        program: config.node_exe.clone(),
        // The bundles ship with source maps: a stack trace in the log then
        // names src/auth.ts and a line in it, not character 1,204,331 of a
        // generated file.
        args: vec!["--enable-source-maps".into(), bundle.into()],
        cwd: config.auth_dir.clone(),
        env: env.clone(),
    };

    ChildSpec {
        name: "auth".to_string(),
        run: node("index.cjs"),
        // What the container's entrypoint does before every start.
        before_start: Some(node("migrate.cjs")),
        health: Some(Health::Http(format!("http://{}/health", config.auth_addr))),
        graceful_stop: None,
        restart: RestartPolicy::default(),
        output: LineFormat::Plain,
    }
}

/// Runs `node --version` and refuses anything older than the auth service
/// supports. Done once at startup: without it a missing or ancient Node shows
/// up as the auth service failing to start, once a minute, forever.
pub async fn check_node(node: &Path) -> anyhow::Result<String> {
    let output = tokio::time::timeout(
        Duration::from_secs(15),
        tokio::process::Command::new(node).arg("--version").output(),
    )
    .await
    .with_context(|| format!("{} --version did not answer", node.display()))?
    .with_context(|| {
        format!(
            "Node.js {MIN_NODE_MAJOR} or newer is required and was not found at {:?}. \
             Install it from nodejs.org, or set NODE_EXE to node.exe's full path",
            node.display().to_string()
        )
    })?;

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    match node_major(&version) {
        Some(major) if major >= MIN_NODE_MAJOR => Ok(version),
        Some(_) => bail!(
            "Node.js {MIN_NODE_MAJOR} or newer is required; {} is {version}",
            node.display()
        ),
        None => bail!(
            "{} --version answered {version:?}, which does not look like Node.js",
            node.display()
        ),
    }
}

/// `v22.22.0` -> 22.
fn node_major(version: &str) -> Option<u32> {
    version.strip_prefix('v')?.split('.').next()?.parse().ok()
}

fn pair(key: &str, value: impl Into<String>) -> (String, String) {
    (key.to_string(), value.into())
}

fn parent_dir(path: &Path) -> std::path::PathBuf {
    path.parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| ".".into())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::*;
    use crate::config::Database;

    const DB: &str = "postgres://nexus@127.0.0.1:15432/nexus";

    fn config() -> Config {
        Config {
            listen: "127.0.0.1:5173".parse().unwrap(),
            web_dir: PathBuf::from(r"C:\Nexus\web"),
            api_addr: "127.0.0.1:18080".parse().unwrap(),
            auth_addr: "127.0.0.1:13010".parse().unwrap(),
            api_exe: PathBuf::from(r"C:\Nexus\bin\integration-api.exe"),
            auth_dir: PathBuf::from(r"C:\Nexus\auth"),
            node_exe: PathBuf::from(r"C:\Program Files\nodejs\node.exe"),
            data_dir: PathBuf::from("C:/ProgramData/Nexus"),
            database: Database::External {
                url: DB.to_string(),
            },
            app_url: "https://nexus.brandkontoret.se".to_string(),
            auth_secret: "s3cret".to_string(),
            api_log_filter: "integration_api=info,warn".to_string(),
        }
    }

    fn health_url(spec: &ChildSpec) -> Option<&str> {
        match &spec.health {
            Some(Health::Http(url)) => Some(url),
            _ => None,
        }
    }

    fn env(spec: &CommandSpec) -> HashMap<&str, &str> {
        spec.env
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
    }

    #[test]
    fn api_listens_on_loopback_and_fetches_keys_from_auth_directly() {
        let spec = api(&config(), DB);
        let env = env(&spec.run);

        assert_eq!(env["HOST"], "127.0.0.1");
        assert_eq!(env["PORT"], "18080");
        assert_eq!(env["AUTH_JWKS_URL"], "http://127.0.0.1:13010/api/auth/jwks");
        assert_eq!(env["APP_URL"], "https://nexus.brandkontoret.se");
        assert_eq!(
            env["DATABASE_URL"],
            "postgres://nexus@127.0.0.1:15432/nexus"
        );
        assert_eq!(env["RUST_LOG"], "integration_api=info,warn");
        assert_eq!(
            health_url(&spec),
            Some("http://127.0.0.1:18080/api/v1/health")
        );
    }

    #[test]
    fn api_runs_from_its_own_folder() {
        let spec = api(&config(), DB);
        assert_eq!(spec.run.cwd, PathBuf::from(r"C:\Nexus\bin"));
    }

    #[test]
    fn auth_migrates_before_every_start() {
        let spec = auth(&config(), DB);

        let before = spec.before_start.expect("auth must migrate first");
        assert_eq!(before.args, ["--enable-source-maps", "migrate.cjs"]);
        assert_eq!(spec.run.args, ["--enable-source-maps", "index.cjs"]);
        // The migration talks to the same database with the same secret.
        assert_eq!(env(&before), env(&spec.run));
    }

    #[test]
    fn auth_listens_on_loopback_with_the_secret() {
        let spec = auth(&config(), DB);
        let env = env(&spec.run);

        assert_eq!(
            spec.run.program,
            PathBuf::from(r"C:\Program Files\nodejs\node.exe")
        );
        assert_eq!(spec.run.cwd, PathBuf::from(r"C:\Nexus\auth"));
        assert_eq!(env["HOST"], "127.0.0.1");
        assert_eq!(env["PORT"], "13010");
        assert_eq!(env["AUTH_SECRET"], "s3cret");
        assert_eq!(health_url(&spec), Some("http://127.0.0.1:13010/health"));
    }

    #[test]
    fn node_versions_are_read_by_major() {
        assert_eq!(node_major("v22.22.0"), Some(22));
        assert_eq!(node_major("v24.1.3"), Some(24));
        assert_eq!(node_major("v18.20.4"), Some(18));
        assert_eq!(node_major("22.1.0"), None);
        assert_eq!(node_major("Python 3.12"), None);
    }

    #[tokio::test]
    async fn missing_node_says_how_to_fix_it() {
        let err = check_node(Path::new(r"C:\definitely\not\here\node.exe"))
            .await
            .unwrap_err();
        let message = format!("{err:#}");

        assert!(message.contains("NODE_EXE"), "{message}");
    }
}

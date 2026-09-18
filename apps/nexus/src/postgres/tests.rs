//! Unit tests for what can be checked without PostgreSQL, and one lifecycle
//! test against the real binaries.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::*;
use crate::config::EmbeddedPostgres;
use crate::supervisor::{self, KillOnClose};

fn embedded(pg_dir: &Path, data_dir: &Path, addr: SocketAddr) -> Embedded {
    Embedded::new(
        &EmbeddedPostgres {
            pg_dir: pg_dir.to_path_buf(),
            addr,
        },
        data_dir,
    )
}

fn sample() -> Embedded {
    embedded(
        Path::new(r"C:\Nexus\pgsql"),
        Path::new(r"C:\ProgramData\Nexus"),
        "127.0.0.1:15432".parse().unwrap(),
    )
}

#[test]
fn server_listens_on_loopback_tcp_only() {
    let spec = sample().child_spec();
    let args = spec.run.args.join(" ");

    assert!(args.contains("-p 15432"), "{args}");
    assert!(args.contains("listen_addresses=127.0.0.1"), "{args}");
    assert!(args.contains("unix_socket_directories="), "{args}");
    assert_eq!(
        spec.run.program,
        PathBuf::from(r"C:\Nexus\pgsql\bin").join(exe("postgres"))
    );
}

#[test]
fn readiness_is_pg_isready_and_stopping_is_a_fast_shutdown() {
    let spec = sample().child_spec();

    let Some(Health::Command(check)) = &spec.health else {
        panic!("expected a command health check");
    };
    assert!(check.program.ends_with(exe("pg_isready")));
    assert!(check.args.join(" ").contains("-h 127.0.0.1 -p 15432"));

    let stop = spec.graceful_stop.expect("postgres must stop gracefully");
    assert!(stop.command.program.ends_with(exe("pg_ctl")));
    assert!(stop.command.args.join(" ").contains("stop"));
    assert!(stop.command.args.join(" ").contains("-m fast"));
}

#[test]
fn every_tool_speaks_english() {
    let spec = sample().child_spec();
    let english = ("LC_MESSAGES".to_string(), "C".to_string());

    assert!(spec.run.env.contains(&english));
    assert!(spec.graceful_stop.unwrap().command.env.contains(&english));
}

#[test]
fn database_url_names_the_compose_database() {
    let url = sample().database_url(&Password("abc123".to_string()));
    assert_eq!(
        url,
        "postgres://postgres:abc123@127.0.0.1:15432/integration_observability"
    );
}

#[test]
fn passwords_are_long_random_hex_and_never_printed() {
    let a = Password::generate().unwrap();
    let b = Password::generate().unwrap();

    assert_eq!(a.expose().len(), 64);
    assert!(a.expose().chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(a.expose(), b.expose());
    assert!(!format!("{a:?}").contains(a.expose()));
}

#[test]
fn an_elevated_console_is_told_what_to_do_instead() {
    // What postgres.exe prints, verbatim, when started with administrator rights.
    let stderr = "Execution of PostgreSQL by a user with administrative permissions is not\n\
                  permitted.\n\
                  The server must be started under an unprivileged user ID to prevent\n\
                  possible system security compromises.";

    let message = explain_refusal(Path::new(r"C:\ProgramData\Nexus\postgres"), stderr);

    assert!(
        message.contains(r#"not "Run as administrator""#),
        "{message}"
    );
    assert!(message.contains("Windows service"), "{message}");
}

#[test]
fn other_refusals_pass_through_with_the_folder() {
    let message = explain_refusal(
        Path::new("/data/postgres"),
        "FATAL: database files are incompatible with server",
    );

    assert!(
        message.contains("/data/postgres") && message.contains("incompatible"),
        "{message}"
    );
}

#[tokio::test]
async fn a_folder_with_something_else_in_it_is_never_initialised_over() {
    let data = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(data.path().join("postgres")).unwrap();
    std::fs::write(data.path().join("postgres").join("keep-me.txt"), "precious").unwrap();

    let pg = embedded(
        Path::new("unused"),
        data.path(),
        "127.0.0.1:15432".parse().unwrap(),
    );
    let err = pg.prepare().await.unwrap_err().to_string();

    assert!(err.contains("not empty"), "{err}");
    assert!(data.path().join("postgres").join("keep-me.txt").exists());
}

#[tokio::test]
async fn an_existing_database_without_its_password_file_says_what_to_restore() {
    let data = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(data.path().join("postgres")).unwrap();
    std::fs::write(data.path().join("postgres").join("PG_VERSION"), "18").unwrap();

    let pg = embedded(
        Path::new("unused"),
        data.path(),
        "127.0.0.1:15432".parse().unwrap(),
    );
    let err = format!("{:#}", pg.prepare().await.unwrap_err());

    assert!(err.contains("postgres.password"), "{err}");
}

/// Creates a cluster, runs it under the supervisor, stores a row, stops it
/// gracefully, starts it again and finds the row. Needs real binaries:
///
/// ```text
/// $env:NEXUS_TEST_PG_DIR = "C:\path\to\pgsql"
/// cargo test -- --ignored
/// ```
#[tokio::test]
#[ignore = "needs PostgreSQL binaries: set NEXUS_TEST_PG_DIR and run with --ignored"]
async fn full_lifecycle_against_real_postgres() {
    let pg_dir = std::env::var("NEXUS_TEST_PG_DIR")
        .map(PathBuf::from)
        .expect("NEXUS_TEST_PG_DIR must point at a PostgreSQL 18 folder");
    let data = tempfile::tempdir().unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let pg = embedded(
        &pg_dir,
        data.path(),
        SocketAddr::from(([127, 0, 0, 1], port)),
    );

    // First start: initdb, a new password, a new database.
    let password = pg.prepare().await.expect("first prepare");
    assert!(data.path().join("postgres.password").is_file());
    let first = run_until_ready(&pg).await;
    pg.ensure_database(&password)
        .await
        .expect("create database");
    pg.ensure_database(&password)
        .await
        .expect("ensure is idempotent");
    sql(
        &pg,
        &password,
        "CREATE TABLE survives (note text); INSERT INTO survives VALUES ('still here')",
    )
    .await;
    stop(first).await;

    // A clean stop removes the lock file; a kill leaves it behind.
    assert!(
        !pg.pgdata().join("postmaster.pid").exists(),
        "postmaster.pid left behind: the server was killed, not stopped"
    );

    // Second start: same password from the file, data intact.
    let again = pg.prepare().await.expect("second prepare");
    assert_eq!(again.expose(), password.expose());
    let second = run_until_ready(&pg).await;
    let note = sql(&pg, &again, "SELECT note FROM survives").await;
    assert_eq!(note.trim(), "still here");

    // And the password is required.
    let wrong = pg.psql(&Password("wrong".to_string()), "SELECT 1").await;
    assert!(wrong.is_err(), "connected without the right password");

    stop(second).await;
}

struct Running {
    stop: CancellationToken,
    handle: tokio::task::JoinHandle<()>,
}

async fn run_until_ready(pg: &Embedded) -> Running {
    let stop = CancellationToken::new();
    let (ready_tx, mut ready) = watch::channel(false);
    let spec = pg.child_spec();
    let token = stop.clone();
    let handle = tokio::spawn(async move {
        let job = KillOnClose::new().unwrap();
        supervisor::supervise(
            spec,
            token,
            ready_tx,
            &job,
            crate::front_door::http_client(),
        )
        .await;
    });
    tokio::time::timeout(Duration::from_secs(60), ready.wait_for(|r| *r))
        .await
        .expect("postgres did not become ready within a minute")
        .unwrap();
    Running { stop, handle }
}

async fn stop(running: Running) {
    running.stop.cancel();
    tokio::time::timeout(Duration::from_secs(120), running.handle)
        .await
        .expect("postgres did not stop")
        .unwrap();
}

/// Runs SQL against Nexus's database rather than `postgres`.
async fn sql(pg: &Embedded, password: &Password, statement: &str) -> String {
    let host = pg.addr().ip().to_string();
    let port = pg.addr().port().to_string();
    let output = pg
        .tool("psql")
        .args([
            "-h",
            &host,
            "-p",
            &port,
            "-U",
            "postgres",
            "-d",
            DATABASE,
            "-X",
            "-A",
            "-t",
            "-v",
            "ON_ERROR_STOP=1",
            "-c",
            statement,
        ])
        .env("PGPASSWORD", password.expose())
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

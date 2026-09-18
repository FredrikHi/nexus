//! The supervisor against real child processes.
//!
//! The children are tiny shell scripts that append a line to a file each time
//! they run, so a test can count restarts and see ordering from outside rather
//! than trusting the supervisor's own account of what it did.

use std::path::Path;
use std::time::{Duration, Instant};

use axum::routing::get;
use axum::Router;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::*;

/// Delays short enough that a test sees several restarts within a second.
fn quick() -> RestartPolicy {
    RestartPolicy {
        initial_delay: Duration::from_millis(20),
        max_delay: Duration::from_millis(50),
        stable_after: Duration::from_secs(30),
    }
}

/// A shell command, run in `dir`.
#[cfg(windows)]
fn shell(dir: &Path, script: &str) -> CommandSpec {
    CommandSpec {
        program: "cmd".into(),
        args: vec!["/C".into(), script.into()],
        cwd: dir.to_path_buf(),
        env: Vec::new(),
    }
}

#[cfg(not(windows))]
fn shell(dir: &Path, script: &str) -> CommandSpec {
    CommandSpec {
        program: "sh".into(),
        args: vec!["-c".into(), script.into()],
        cwd: dir.to_path_buf(),
        env: Vec::new(),
    }
}

/// Appends `word` to `file`, then exits with `code`.
fn record_and_exit(word: &str, file: &str, code: u8) -> String {
    if cfg!(windows) {
        format!("echo {word}>>{file}& exit /b {code}")
    } else {
        format!("echo {word} >> {file}; exit {code}")
    }
}

/// Appends `word` to `file`, then stays running for a minute.
fn record_and_stay(word: &str, file: &str) -> String {
    if cfg!(windows) {
        format!("echo {word}>>{file}& ping -n 60 127.0.0.1 >nul")
    } else {
        format!("echo {word} >> {file}; sleep 60")
    }
}

fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Polls `condition` until it holds, failing the test after `limit`.
async fn eventually(limit: Duration, what: &str, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + limit;
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for: {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn spec(name: &str, run: CommandSpec) -> ChildSpec {
    ChildSpec {
        name: name.to_string(),
        run,
        before_start: None,
        health: None,
        graceful_stop: None,
        restart: quick(),
        output: LineFormat::Plain,
    }
}

/// Runs the supervisor in the background. Returns the stop token, the ready
/// flag, and a handle that finishes when the supervisor has returned.
fn run(
    spec: ChildSpec,
) -> (
    CancellationToken,
    watch::Receiver<bool>,
    tokio::task::JoinHandle<()>,
) {
    let stop = CancellationToken::new();
    let (ready_tx, ready_rx) = watch::channel(false);
    let token = stop.clone();
    let handle = tokio::spawn(async move {
        let job = KillOnClose::new().unwrap();
        supervise(
            spec,
            token,
            ready_tx,
            &job,
            crate::front_door::http_client(),
        )
        .await;
    });
    (stop, ready_rx, handle)
}

async fn stop_and_join(stop: CancellationToken, handle: tokio::task::JoinHandle<()>) {
    stop.cancel();
    tokio::time::timeout(Duration::from_secs(10), handle)
        .await
        .expect("supervisor did not stop within 10 seconds")
        .unwrap();
}

#[test]
fn backoff_doubles_up_to_the_cap_and_starts_over_after_reset() {
    let mut backoff = Backoff::new(RestartPolicy {
        initial_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(10),
        stable_after: Duration::from_secs(60),
    });

    let delays: Vec<u64> = (0..6).map(|_| backoff.next_delay().as_secs()).collect();
    assert_eq!(delays, [1, 2, 4, 8, 10, 10]);

    backoff.reset();
    assert_eq!(backoff.next_delay(), Duration::from_secs(1));
}

#[tokio::test]
async fn a_crashing_child_is_started_again() {
    let dir = tempfile::tempdir().unwrap();
    let runs = dir.path().join("runs.txt");

    let (stop, _, handle) = run(spec(
        "crasher",
        shell(dir.path(), &record_and_exit("run", "runs.txt", 3)),
    ));

    eventually(Duration::from_secs(15), "three runs", || {
        lines(&runs).len() >= 3
    })
    .await;
    stop_and_join(stop, handle).await;
}

#[tokio::test]
async fn setup_runs_to_completion_before_the_child_starts() {
    let dir = tempfile::tempdir().unwrap();
    let order = dir.path().join("order.txt");

    let mut spec = spec(
        "service",
        shell(dir.path(), &record_and_stay("run", "order.txt")),
    );
    spec.before_start = Some(shell(dir.path(), &record_and_exit("setup", "order.txt", 0)));
    let (stop, _, handle) = run(spec);

    eventually(Duration::from_secs(15), "the service to start", || {
        lines(&order).len() >= 2
    })
    .await;
    assert_eq!(lines(&order)[..2], ["setup", "run"]);
    stop_and_join(stop, handle).await;
}

#[tokio::test]
async fn failing_setup_keeps_the_child_from_starting_and_is_retried() {
    let dir = tempfile::tempdir().unwrap();
    let setup = dir.path().join("setup.txt");
    let service = dir.path().join("service.txt");

    let mut spec = spec(
        "service",
        shell(dir.path(), &record_and_stay("run", "service.txt")),
    );
    spec.before_start = Some(shell(dir.path(), &record_and_exit("setup", "setup.txt", 1)));
    let (stop, _, handle) = run(spec);

    eventually(Duration::from_secs(15), "setup to be retried", || {
        lines(&setup).len() >= 3
    })
    .await;
    assert!(
        !service.exists(),
        "the service ran although its setup failed"
    );
    stop_and_join(stop, handle).await;
}

#[tokio::test]
async fn stopping_ends_a_running_child_without_waiting_for_it() {
    let dir = tempfile::tempdir().unwrap();
    let runs = dir.path().join("runs.txt");

    let (stop, _, handle) = run(spec(
        "long",
        shell(dir.path(), &record_and_stay("run", "runs.txt")),
    ));
    eventually(Duration::from_secs(15), "the child to start", || {
        runs.exists()
    })
    .await;

    // The child would run for a minute; stop_and_join allows ten seconds.
    stop_and_join(stop, handle).await;
    assert_eq!(
        lines(&runs).len(),
        1,
        "the child was started again while stopping"
    );
}

#[tokio::test]
async fn ready_follows_the_health_url_and_clears_when_the_child_exits() {
    let dir = tempfile::tempdir().unwrap();

    // Stands in for the service's health endpoint.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let health = format!("http://{}/health", listener.local_addr().unwrap());
    let app = Router::new().route("/health", get(|| async { "ok" }));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    // Up for about two seconds, then exits.
    let script = if cfg!(windows) {
        "ping -n 3 127.0.0.1 >nul"
    } else {
        "sleep 2"
    };
    let mut spec = spec("service", shell(dir.path(), script));
    spec.health = Some(Health::Http(health));
    spec.restart.initial_delay = Duration::from_secs(30);
    spec.restart.max_delay = Duration::from_secs(30);
    let (stop, mut ready, handle) = run(spec);

    tokio::time::timeout(Duration::from_secs(10), ready.wait_for(|r| *r))
        .await
        .expect("never became ready")
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), ready.wait_for(|r| !*r))
        .await
        .expect("still ready after the child exited")
        .unwrap();

    stop_and_join(stop, handle).await;
}

#[tokio::test]
async fn a_program_that_does_not_exist_is_retried_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    let spec = spec(
        "ghost",
        CommandSpec {
            program: dir.path().join("no-such-program.exe"),
            args: Vec::new(),
            cwd: dir.path().to_path_buf(),
            env: Vec::new(),
        },
    );

    let (stop, _, handle) = run(spec);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !handle.is_finished(),
        "the supervisor gave up on a missing program"
    );
    stop_and_join(stop, handle).await;
}

#[tokio::test]
async fn a_command_health_check_decides_readiness() {
    let dir = tempfile::tempdir().unwrap();
    let stay = shell(dir.path(), &record_and_stay("run", "runs.txt"));

    let mut passing = spec("passing", stay.clone());
    passing.health = Some(Health::Command(shell(dir.path(), "exit 0")));
    let (stop, mut ready, handle) = run(passing);
    tokio::time::timeout(Duration::from_secs(10), ready.wait_for(|r| *r))
        .await
        .expect("a passing check never made it ready")
        .unwrap();
    stop_and_join(stop, handle).await;

    let mut failing = spec("failing", stay);
    failing.health = Some(Health::Command(shell(dir.path(), "exit 1")));
    let (stop, ready, handle) = run(failing);
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(!*ready.borrow(), "a failing check made it ready");
    stop_and_join(stop, handle).await;
}

#[tokio::test]
async fn stopping_asks_first_and_kills_what_does_not_listen() {
    let dir = tempfile::tempdir().unwrap();
    let asked = dir.path().join("asked.txt");

    // The stop command is recorded but the child ignores it, as a hung
    // database would. The supervisor must still return, having killed it.
    let mut spec = spec(
        "stubborn",
        shell(dir.path(), &record_and_stay("run", "runs.txt")),
    );
    spec.graceful_stop = Some(GracefulStop {
        command: shell(dir.path(), &record_and_exit("asked", "asked.txt", 0)),
        timeout: Duration::from_millis(500),
    });
    let (stop, _, handle) = run(spec);
    eventually(Duration::from_secs(15), "the child to start", || {
        dir.path().join("runs.txt").exists()
    })
    .await;

    stop_and_join(stop, handle).await;
    assert_eq!(
        lines(&asked),
        ["asked"],
        "the stop command was not run once"
    );
}

/// The guarantee the job object exists for: children die with their parent even
/// when the parent never gets to stop them. Dropping the job stands in for this
/// process being killed.
#[cfg(windows)]
#[tokio::test]
async fn children_die_when_the_job_closes() {
    let job = KillOnClose::new().unwrap();
    let mut child = tokio::process::Command::new("ping")
        .args(["-n", "60", "127.0.0.1"])
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    job.adopt(&child).unwrap();

    drop(job);

    tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("the child outlived its job")
        .unwrap();
}

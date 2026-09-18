//! Keeps child processes running: what Docker's `restart: unless-stopped` and
//! the containers' entrypoints did.
//!
//! Generic on purpose. Nothing here knows what the API, the auth service or
//! Postgres is; the modules that do describe them, and this runs whatever it
//! is given.

mod backoff;
mod job;
mod output;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::Request;
use axum::http::Uri;
use tokio::process::{Child, Command};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

pub use backoff::{Backoff, RestartPolicy};
pub use job::KillOnClose;
pub use output::LineFormat;

use crate::front_door::HttpClient;
use output::Stream;

/// One program to run: what, where, and with which extra environment. The
/// child inherits this process's environment, and `env` is set on top of it.
#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
}

/// How to tell that a started service is actually serving.
#[derive(Debug, Clone)]
pub enum Health {
    /// Answers 2xx.
    Http(String),
    /// Exits with code 0. For services with no HTTP endpoint, like Postgres
    /// and `pg_isready`.
    Command(CommandSpec),
}

/// How to stop a service more gently than killing it.
#[derive(Debug, Clone)]
pub struct GracefulStop {
    /// Asks the service to shut down.
    pub command: CommandSpec,
    /// How long the service gets to exit after being asked, before it is
    /// killed anyway.
    pub timeout: Duration,
}

/// A long-running service and how to look after it.
#[derive(Debug, Clone)]
pub struct ChildSpec {
    pub name: String,
    pub run: CommandSpec,
    /// Runs to completion before every start, like a container entrypoint
    /// migrating the schema before it execs the service. If it fails the
    /// service is not started, and both are retried after the backoff.
    pub before_start: Option<CommandSpec>,
    /// Without one, a started process counts as ready.
    pub health: Option<Health>,
    /// Without one, stopping kills the process. Fine for a stateless service;
    /// a database wants to finish its checkpoint.
    pub graceful_stop: Option<GracefulStop>,
    pub restart: RestartPolicy,
    /// How its output is written, for logging each line at its own level.
    pub output: LineFormat,
}

/// Runs `spec` until `stop` is cancelled, starting it again whenever it exits.
///
/// `ready` is true while the current run has passed its health check, and
/// false otherwise, including between a crash and the restart.
pub async fn supervise(
    spec: ChildSpec,
    stop: CancellationToken,
    ready: watch::Sender<bool>,
    job: &KillOnClose,
    client: HttpClient,
) {
    let mut backoff = Backoff::new(spec.restart);

    loop {
        if let Some(before) = &spec.before_start {
            let label = format!("{}-setup", spec.name);
            match run_to_completion(&label, before, spec.output, &stop, job).await {
                Completion::Succeeded => {}
                Completion::Stopped => return,
                Completion::Failed(reason) => {
                    tracing::warn!(child = %spec.name, %reason, "setup step failed, not starting");
                    if wait_unless_stopped(backoff.next_delay(), &stop, &spec.name).await {
                        return;
                    }
                    continue;
                }
            }
        }

        let mut child = match start(&spec.name, &spec.run, spec.output, job) {
            Ok(child) => child,
            Err(err) => {
                tracing::error!(
                    child = %spec.name,
                    program = %spec.run.program.display(),
                    error = %err,
                    "could not start"
                );
                if wait_unless_stopped(backoff.next_delay(), &stop, &spec.name).await {
                    return;
                }
                continue;
            }
        };

        let started = Instant::now();
        let this_run = CancellationToken::new();
        match &spec.health {
            Some(health) => {
                tokio::spawn(report_ready(
                    spec.name.clone(),
                    health.clone(),
                    client.clone(),
                    ready.clone(),
                    started,
                    this_run.clone(),
                ));
            }
            None => {
                ready.send_replace(true);
            }
        }

        // Whichever happens first: the process ends, or we are told to stop.
        let exited = tokio::select! {
            status = child.wait() => Some(status),
            _ = stop.cancelled() => None,
        };
        this_run.cancel();
        ready.send_replace(false);

        let Some(status) = exited else {
            shut_down(&spec, &mut child, job).await;
            return;
        };

        let ran_for = started.elapsed();
        match status {
            Ok(status) => tracing::warn!(
                child = %spec.name,
                exit = %describe(status),
                ran_for = ?ran_for,
                "exited"
            ),
            Err(err) => {
                tracing::error!(child = %spec.name, error = %err, "lost track of the process")
            }
        }
        if ran_for >= spec.restart.stable_after {
            backoff.reset();
        }
        if wait_unless_stopped(backoff.next_delay(), &stop, &spec.name).await {
            return;
        }
    }
}

/// Asks nicely if the spec says how, then kills whatever is still running.
async fn shut_down(spec: &ChildSpec, child: &mut Child, job: &KillOnClose) {
    if let Some(graceful) = &spec.graceful_stop {
        let label = format!("{}-stop", spec.name);
        // A token nobody cancels: the stop we are carrying out is the one that
        // cancelled the supervisor's, and this command must not be cut short
        // by it. The timeout below bounds it instead.
        let never = CancellationToken::new();
        let asked = tokio::time::timeout(
            graceful.timeout,
            run_to_completion(&label, &graceful.command, spec.output, &never, job),
        )
        .await;
        match asked {
            // Nothing cancels `never`, so Stopped cannot happen; listed so the
            // match stays exhaustive if Completion grows.
            Ok(Completion::Succeeded) | Ok(Completion::Stopped) => {}
            Ok(Completion::Failed(reason)) => {
                tracing::warn!(child = %spec.name, %reason, "graceful stop failed")
            }
            Err(_) => tracing::warn!(child = %spec.name, "graceful stop did not finish in time"),
        }

        // Even a successful stop command only asked; give the process the rest
        // of its time to actually exit.
        if tokio::time::timeout(graceful.timeout, child.wait())
            .await
            .is_ok()
        {
            tracing::info!(child = %spec.name, "stopped");
            return;
        }
        tracing::warn!(child = %spec.name, "did not stop when asked, killing it");
    }

    if let Err(err) = child.kill().await {
        tracing::warn!(child = %spec.name, error = %err, "could not stop");
    }
    tracing::info!(child = %spec.name, "stopped");
}

enum Completion {
    Succeeded,
    Failed(String),
    Stopped,
}

async fn run_to_completion(
    label: &str,
    spec: &CommandSpec,
    format: LineFormat,
    stop: &CancellationToken,
    job: &KillOnClose,
) -> Completion {
    let mut child = match start(label, spec, format, job) {
        Ok(child) => child,
        Err(err) => {
            return Completion::Failed(format!("could not start {}: {err}", spec.program.display()))
        }
    };

    let exited = tokio::select! {
        status = child.wait() => Some(status),
        _ = stop.cancelled() => None,
    };

    match exited {
        None => {
            let _ = child.kill().await;
            Completion::Stopped
        }
        Some(Ok(status)) if status.success() => Completion::Succeeded,
        Some(Ok(status)) => Completion::Failed(format!("exited with {}", describe(status))),
        Some(Err(err)) => Completion::Failed(err.to_string()),
    }
}

fn command(spec: &CommandSpec) -> Command {
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .envs(spec.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        // Belt and braces with the job object: covers the Child being dropped
        // without an explicit kill on a path nobody anticipated.
        .kill_on_drop(true);

    // No console of its own, and not in this console's process group. Ctrl+C
    // in the window running nexus.exe would otherwise reach the children at the
    // same moment it reaches us, killing them before the front door has
    // finished the requests it is draining.
    #[cfg(windows)]
    {
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }

    command
}

fn start(
    name: &str,
    spec: &CommandSpec,
    format: LineFormat,
    job: &KillOnClose,
) -> std::io::Result<Child> {
    let mut child = command(spec)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Err(err) = job.adopt(&child) {
        tracing::warn!(
            child = name,
            error = %err,
            "could not tie the process to nexus; it would outlive a crash of nexus"
        );
    }
    if let Some(stdout) = child.stdout.take() {
        output::forward(name.to_string(), stdout, Stream::Stdout, format);
    }
    if let Some(stderr) = child.stderr.take() {
        output::forward(name.to_string(), stderr, Stream::Stderr, format);
    }
    Ok(child)
}

/// Polls `health` until it passes or this run ends.
async fn report_ready(
    name: String,
    health: Health,
    client: HttpClient,
    ready: watch::Sender<bool>,
    started: Instant,
    this_run: CancellationToken,
) {
    let uri = match &health {
        Health::Http(url) => match url.parse::<Uri>() {
            Ok(uri) => Some(uri),
            Err(err) => {
                tracing::error!(child = %name, %url, error = %err, "health url is not valid");
                return;
            }
        },
        Health::Command(_) => None,
    };

    loop {
        let healthy = match (&health, &uri) {
            (Health::Http(_), Some(uri)) => http_ok(&client, uri).await,
            (Health::Command(spec), _) => command_ok(spec).await,
            (Health::Http(_), None) => false,
        };

        // Checked again after the await: the process may have exited while the
        // check was in flight, and a stale "ready" must not outlive it.
        if this_run.is_cancelled() {
            return;
        }
        if healthy {
            tracing::info!(child = %name, after = ?started.elapsed(), "ready");
            ready.send_replace(true);
            return;
        }

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(250)) => {}
            _ = this_run.cancelled() => return,
        }
    }
}

async fn http_ok(client: &HttpClient, uri: &Uri) -> bool {
    let mut request = Request::new(Body::empty());
    *request.uri_mut() = uri.clone();
    let answered = tokio::time::timeout(Duration::from_secs(2), client.request(request)).await;
    matches!(answered, Ok(Ok(response)) if response.status().is_success())
}

/// Runs a check command silently. Its output would otherwise be a line in the
/// log four times a second ("no response") until the service is up.
async fn command_ok(spec: &CommandSpec) -> bool {
    let status = command(spec)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    matches!(
        tokio::time::timeout(Duration::from_secs(5), status).await,
        Ok(Ok(status)) if status.success()
    )
}

/// Sleeps for `delay`, returning early with `true` if told to stop meanwhile.
async fn wait_unless_stopped(delay: Duration, stop: &CancellationToken, name: &str) -> bool {
    tracing::info!(child = %name, delay = ?delay, "starting again");
    tokio::select! {
        _ = tokio::time::sleep(delay) => false,
        _ = stop.cancelled() => true,
    }
}

fn describe(status: ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("code {code}"),
        None => "a signal".to_string(),
    }
}

//! What runs when Windows starts the Nexus service.
//!
//! Windows calls `main` with `service --config <file>`, `main` hands control to
//! the service dispatcher, and the dispatcher calls `service_main` on a thread
//! of its own. From there it is `app::run`, the same as in a console, with the
//! service control manager's stop in place of Ctrl+C.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::Context;
use tokio_util::sync::CancellationToken;
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{
    self, ServiceControlHandlerResult, ServiceStatusHandle,
};
use windows_service::{define_windows_service, service_dispatcher};

use super::SERVICE_NAME;
use crate::config::Config;
use crate::{app, logging};

/// The dispatcher calls `service_main` with the arguments given to *start*
/// the service, not the ones Windows launched the process with, so the config
/// path is handed across here.
static CONFIG_FILE: OnceLock<PathBuf> = OnceLock::new();

define_windows_service!(ffi_service_main, service_main);

pub fn run(config_file: PathBuf) -> anyhow::Result<()> {
    let _ = CONFIG_FILE.set(config_file);
    service_dispatcher::start(SERVICE_NAME, ffi_service_main).context(
        "`nexus service` is started by Windows, not by hand. To run in this console, use `nexus run`",
    )
}

fn service_main(_start_arguments: Vec<OsString>) {
    // Any error has already been logged and reported to Windows by now; there
    // is nobody else to give it to.
    let _ = run_service();
}

fn run_service() -> anyhow::Result<()> {
    let config_file = CONFIG_FILE
        .get()
        .context("no config file was given")?
        .clone();

    // Logging first, so that even a config file that fails to parse leaves its
    // reason in a file rather than nowhere.
    let log_dir = config_file.parent().map(|dir| dir.join("logs"));
    let _logging = logging::init(false, log_dir.as_deref())?;
    tracing::info!(config = %config_file.display(), version = env!("CARGO_PKG_VERSION"), "service starting");

    let stop = CancellationToken::new();
    let on_control = stop.clone();
    let status = service_control_handler::register(SERVICE_NAME, move |control| match control {
        // Preshutdown rather than Shutdown: on a restart of the server, Windows
        // gives a service that asks for preshutdown minutes to finish, where
        // shutdown gets seconds. That is PostgreSQL's checkpoint, not a crash.
        ServiceControl::Stop | ServiceControl::Preshutdown => {
            on_control.cancel();
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    })?;

    report(
        &status,
        ServiceState::StartPending,
        ServiceControlAccept::empty(),
        1,
        ServiceExitCode::NO_ERROR,
    );

    let result = (|| -> anyhow::Result<()> {
        let config = Config::load(Some(&config_file))?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("could not start the async runtime")?;

        report(
            &status,
            ServiceState::Running,
            ServiceControlAccept::STOP | ServiceControlAccept::PRESHUTDOWN,
            0,
            ServiceExitCode::NO_ERROR,
        );

        runtime.block_on(async {
            let progress = tokio::spawn(report_stopping(status, stop.clone()));
            let result = app::run(config, stop.clone().cancelled_owned()).await;
            progress.abort();
            result
        })
    })();

    let exit_code = match &result {
        Ok(()) => ServiceExitCode::NO_ERROR,
        Err(err) => {
            tracing::error!(error = %format!("{err:#}"), "service stopped with an error");
            ServiceExitCode::ServiceSpecific(1)
        }
    };
    tracing::info!("service stopped");
    report(
        &status,
        ServiceState::Stopped,
        ServiceControlAccept::empty(),
        0,
        exit_code,
    );
    result
}

/// While stopping, tells Windows every few seconds that the stop is still
/// making progress. Without it, a PostgreSQL checkpoint that takes longer than
/// the wait hint looks like a hung service.
async fn report_stopping(status: ServiceStatusHandle, stop: CancellationToken) {
    stop.cancelled().await;
    let mut checkpoint = 1;
    loop {
        report(
            &status,
            ServiceState::StopPending,
            ServiceControlAccept::empty(),
            checkpoint,
            ServiceExitCode::NO_ERROR,
        );
        checkpoint += 1;
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

fn report(
    status: &ServiceStatusHandle,
    state: ServiceState,
    accepts: ServiceControlAccept,
    checkpoint: u32,
    exit_code: ServiceExitCode,
) {
    let result = status.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: accepts,
        exit_code,
        checkpoint,
        // Only meaningful while pending: the longest Windows should wait for
        // the next report before concluding the service has hung.
        wait_hint: match state {
            ServiceState::StartPending | ServiceState::StopPending => Duration::from_secs(30),
            _ => Duration::ZERO,
        },
        process_id: None,
    });
    if let Err(err) = result {
        tracing::warn!(error = %err, ?state, "could not report the service state to Windows");
    }
}

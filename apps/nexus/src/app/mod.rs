//! Running Nexus: the database, the API, the auth service and the front door,
//! started in that order and stopped in reverse.
//!
//! The console (`nexus run`) and the Windows service run exactly this, and
//! differ only in what `shutdown` waits for: Ctrl+C, or the service control
//! manager's stop.

#[cfg(test)]
mod tests;

use std::future::Future;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Context;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::config::{Config, Database};
use crate::supervisor::{self, KillOnClose};
use crate::{front_door, postgres, services};

/// Runs until `shutdown` resolves, then stops everything cleanly and returns.
pub async fn run(
    config: Config,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    let node = services::check_node(&config.node_exe).await?;
    tracing::info!(node = %node, "found Node.js");

    // Everything that can be refused is refused before anything is started, so
    // a port clash never leaves half of Nexus running.
    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .with_context(|| format!("cannot listen on {} (LISTEN_ADDR)", config.listen))?;
    ensure_free(config.api_addr, "API_ADDR")?;
    ensure_free(config.auth_addr, "AUTH_ADDR")?;

    // The database: someone else's, or ours. Ours is created on first start,
    // which is also where "cannot run as an administrator" surfaces.
    let embedded = match &config.database {
        Database::External { .. } => {
            tracing::info!("using the PostgreSQL server in DATABASE_URL");
            None
        }
        Database::Embedded(settings) => {
            ensure_free(settings.addr, "POSTGRES_ADDR")?;
            let pg = postgres::Embedded::new(settings, &config.data_dir);
            tracing::info!(
                listen = %pg.addr(),
                data = %pg.pgdata().display(),
                "using the embedded PostgreSQL"
            );
            let password = pg.prepare().await?;
            Some((pg, password))
        }
    };
    let database_url = match (&config.database, &embedded) {
        (Database::External { url }, _) => url.clone(),
        (Database::Embedded(_), Some((pg, password))) => pg.database_url(password),
        (Database::Embedded(_), None) => unreachable!("prepared just above"),
    };

    let job = KillOnClose::new()?;
    let client = front_door::http_client();

    // Stopped in this order, each once the one before has finished: the front
    // door, so no new requests arrive; the API and auth, so nothing is mid-query;
    // then the database.
    let stop_apps = CancellationToken::new();
    let stop_database = CancellationToken::new();

    let (database_ready_tx, database_ready) = watch::channel(embedded.is_none());
    let (api_ready_tx, api_ready) = watch::channel(false);
    let (auth_ready_tx, auth_ready) = watch::channel(false);

    let database = async {
        if let Some((pg, _)) = &embedded {
            supervisor::supervise(
                pg.child_spec(),
                stop_database.clone(),
                database_ready_tx,
                &job,
                client.clone(),
            )
            .await;
        }
    };

    let apps = async {
        let started = start_apps_when_database_is_ready(
            database_ready,
            embedded.as_ref().map(|(pg, password)| (pg, password)),
            &stop_apps,
        )
        .await;
        if started {
            tokio::join!(
                supervisor::supervise(
                    services::api(&config, &database_url),
                    stop_apps.clone(),
                    api_ready_tx,
                    &job,
                    client.clone(),
                ),
                supervisor::supervise(
                    services::auth(&config, &database_url),
                    stop_apps.clone(),
                    auth_ready_tx,
                    &job,
                    client.clone(),
                ),
            );
        }
        // The apps are down (or never came up): the database can go.
        stop_database.cancel();
    };

    let app = front_door::router(
        &front_door::Settings::new(config.web_dir.clone(), config.api_addr, config.auth_addr),
        client.clone(),
    );
    let front_door = async {
        tracing::info!(listen = %config.listen, "front door open");
        // `with_connect_info` records each connection's remote address so the
        // proxy can pass it on as X-Forwarded-For.
        let served = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            shutdown.await;
            tracing::info!("stop requested");
        })
        .await;

        // Only once the front door has finished the requests it was handling
        // do the services behind it go, so none of those requests is cut off.
        tracing::info!("front door closed, stopping services");
        stop_apps.cancel();
        served
    };

    let announce = announce_when_ready(api_ready, auth_ready, config.app_url.clone());

    let (served, (), (), ()) = tokio::join!(front_door, apps, database, announce);
    served?;

    tracing::info!("stopped");
    Ok(())
}

/// Waits for the database's first start, then makes sure Nexus's database
/// exists in it. Returns false if told to stop before that happened.
///
/// Only the first start is waited for. If PostgreSQL restarts later, the API
/// and auth keep running and reconnect on their own; restarting them too would
/// turn one outage into three.
async fn start_apps_when_database_is_ready(
    mut database_ready: watch::Receiver<bool>,
    embedded: Option<(&postgres::Embedded, &postgres::Password)>,
    stop: &CancellationToken,
) -> bool {
    tokio::select! {
        ready = database_ready.wait_for(|ready| *ready) => {
            if ready.is_err() {
                return false;
            }
        }
        _ = stop.cancelled() => return false,
    }

    let Some((pg, password)) = embedded else {
        return true;
    };
    loop {
        match pg.ensure_database(password).await {
            Ok(()) => return true,
            Err(err) => {
                tracing::error!(error = %format!("{err:#}"), "could not create the database, retrying");
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                    _ = stop.cancelled() => return false,
                }
            }
        }
    }
}

/// Checks nothing is listening on `addr` yet, by binding it and letting go.
/// The child still has to win the port for itself a moment later; this is here
/// for the message, which names the setting to change.
fn ensure_free(addr: SocketAddr, key: &str) -> anyhow::Result<()> {
    std::net::TcpListener::bind(addr).with_context(|| {
        format!(
            "port {} is already in use; set {key} to a free loopback port",
            addr.port()
        )
    })?;
    Ok(())
}

/// One line once both services answer, which is the line someone watching the
/// log is waiting for. Each service still logs its own "ready".
async fn announce_when_ready(
    mut api: watch::Receiver<bool>,
    mut auth: watch::Receiver<bool>,
    app_url: String,
) {
    let both = async {
        api.wait_for(|ready| *ready).await?;
        auth.wait_for(|ready| *ready).await?;
        Ok::<_, watch::error::RecvError>(())
    };
    // An error means a supervisor ended first: we are shutting down, and
    // there is nothing to announce.
    if both.await.is_ok() {
        tracing::info!("Nexus is ready at {app_url}");
    }
}

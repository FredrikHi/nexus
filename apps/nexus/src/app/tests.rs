//! The whole of Nexus against a real release layout: started, used, and
//! stopped the way the service's stop and Ctrl+C both stop it.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tokio::sync::oneshot;

use super::run;
use crate::config::Config;

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn get(url: &str) -> Option<u16> {
    let client = crate::front_door::http_client();
    let mut request = axum::extract::Request::new(axum::body::Body::empty());
    *request.uri_mut() = url.parse().ok()?;
    let response = tokio::time::timeout(Duration::from_secs(5), client.request(request))
        .await
        .ok()?
        .ok()?;
    Some(response.status().as_u16())
}

async fn until_ok(url: &str, limit: Duration) {
    let deadline = Instant::now() + limit;
    while get(url).await != Some(200) {
        assert!(
            Instant::now() < deadline,
            "{url} did not answer 200 in time"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// Needs an assembled release (apps/nexus/scripts/assemble.ps1) and Node:
///
/// ```text
/// $env:NEXUS_TEST_LAYOUT = "C:\path\to\assembled\release"
/// cargo test -- --ignored
/// ```
#[tokio::test]
#[ignore = "needs an assembled release: set NEXUS_TEST_LAYOUT and run with --ignored"]
async fn stops_cleanly_when_asked() {
    let layout = std::env::var("NEXUS_TEST_LAYOUT")
        .map(PathBuf::from)
        .expect("NEXUS_TEST_LAYOUT must point at an assembled release");
    let data = tempfile::tempdir().unwrap();

    let ports = [free_port(), free_port(), free_port(), free_port()];
    let vars = std::collections::HashMap::from([
        (
            "AUTH_SECRET",
            "0123456789abcdef0123456789abcdef".to_string(),
        ),
        ("DATA_DIR", data.path().display().to_string()),
        ("LISTEN_ADDR", format!("127.0.0.1:{}", ports[0])),
        ("API_ADDR", format!("127.0.0.1:{}", ports[1])),
        ("AUTH_ADDR", format!("127.0.0.1:{}", ports[2])),
        ("POSTGRES_ADDR", format!("127.0.0.1:{}", ports[3])),
        ("APP_URL", format!("http://localhost:{}", ports[0])),
    ]);
    let config = Config::from_lookup(&layout, None, |key| vars.get(key).cloned()).unwrap();
    let front = format!("http://127.0.0.1:{}", ports[0]);

    let (stop, stopped) = oneshot::channel::<()>();
    let running = tokio::spawn(run(config, async {
        let _ = stopped.await;
    }));

    // Everything up: through the front door, into the API, down to Postgres,
    // and the auth service's keys.
    until_ok(
        &format!("{front}/api/v1/health/ready"),
        Duration::from_secs(180),
    )
    .await;
    until_ok(&format!("{front}/api/auth/jwks"), Duration::from_secs(60)).await;

    let asked = Instant::now();
    stop.send(()).unwrap();
    let result = tokio::time::timeout(Duration::from_secs(180), running)
        .await
        .expect("run did not return within three minutes of being asked to stop")
        .unwrap();
    result.expect("run returned an error while stopping");
    eprintln!("stopped {:?} after being asked", asked.elapsed());

    // PostgreSQL removes its lock file only on a clean shutdown.
    let pid_file = data.path().join("postgres").join("postmaster.pid");
    assert!(
        !pid_file.exists(),
        "postmaster.pid left behind: PostgreSQL was killed, not stopped"
    );

    // And nothing is left listening.
    for port in ports {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        assert!(
            TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_err(),
            "port {port} is still open after stopping"
        );
    }
}

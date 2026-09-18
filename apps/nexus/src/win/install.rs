//! `nexus install` and `nexus uninstall`.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use rand::rngs::SysRng;
use rand::TryRng;
use windows_service::service::{
    ServiceAccess, ServiceAction, ServiceActionType, ServiceErrorControl, ServiceFailureActions,
    ServiceFailureResetPeriod, ServiceInfo, ServiceStartType, ServiceState, ServiceType,
};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

use super::{acl, paths, SERVICE_ACCOUNT, SERVICE_NAME};
use crate::cli::InstallArgs;
use crate::config::{self, Config};
use crate::config_file::{self, Settings};
use crate::services;

/// Windows' "access denied", and "the service does not exist".
const ERROR_ACCESS_DENIED: i32 = 5;
const ERROR_SERVICE_DOES_NOT_EXIST: i32 = 1060;

pub async fn install(args: InstallArgs) -> anyhow::Result<()> {
    let exe = paths::resolve(&std::env::current_exe()?)?;
    let exe_dir = exe
        .parent()
        .context("nexus.exe has no folder")?
        .to_path_buf();
    paths::refuse_user_profile(
        "nexus.exe",
        &exe_dir,
        r"Move the Nexus folder somewhere every account can read, such as C:\Program Files\Nexus, and install from there.",
    )?;

    let config_file = match &args.config {
        Some(path) => std::path::absolute(path).context("cannot resolve --config")?,
        None => config::default_config_file()?,
    };
    let config_dir = config_file
        .parent()
        .context("--config has no folder")?
        .to_path_buf();

    // Start from what an earlier install wrote. A reinstall must never replace
    // AUTH_SECRET: that signs everyone out, and the auth service's signing keys
    // are encrypted with it, so the old ones become unreadable.
    let mut settings = if config_file.is_file() {
        println!("Using the existing settings in {}", config_file.display());
        config_file::read(&config_file)?
    } else {
        Settings::new()
    };
    apply_arguments(&mut settings, &args)?;

    // The Node the service will run: resolved through any symlink to the real
    // file, since that is what the service account needs to be able to read.
    let node = match (&args.node, settings.get("NODE_EXE")) {
        (Some(node), _) => node.clone(),
        (None, Some(node)) => PathBuf::from(node),
        (None, None) => paths::find_on_path("node.exe").context(
            "node.exe was not found on PATH. Install Node.js 22 or newer from nodejs.org, or pass --node",
        )?,
    };
    let node = paths::resolve(&node)?;
    paths::refuse_user_profile(
        "Node.js",
        &node,
        "Install Node.js for all users from nodejs.org, or pass --node with a copy outside any \
         user profile. nvm for Windows installs into the profile of whoever ran it.",
    )?;
    let version = services::check_node(&node).await?;
    println!("Node.js {version} at {}", node.display());
    settings.insert("NODE_EXE".into(), node.display().to_string());

    // Everything the service will check at start, checked now, before anything
    // on this machine has been changed.
    let config = Config::from_lookup(&exe_dir, Some(&config_dir), |key| {
        settings.get(key).filter(|v| !v.is_empty()).cloned()
    })
    .context("the settings would not start")?;

    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )
    .map_err(explain)?;
    if manager
        .open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS)
        .is_ok()
    {
        bail!(
            "Nexus is already installed. Run `nexus uninstall` first; settings and data are kept."
        );
    }

    std::fs::create_dir_all(&config_dir)
        .with_context(|| format!("cannot create {}", config_dir.display()))?;

    let service = manager
        .create_service(
            &service_info(&exe, &config_file),
            ServiceAccess::QUERY_STATUS
                | ServiceAccess::START
                | ServiceAccess::STOP
                | ServiceAccess::CHANGE_CONFIG
                | ServiceAccess::DELETE,
        )
        .map_err(explain)?;

    // From here a failure takes the service away again, rather than leaving a
    // registered service with no settings behind it.
    let configured = (|| -> anyhow::Result<()> {
        service.set_description(
            "Nexus integration observability: PostgreSQL, the API, the auth service and the web app.",
        )?;
        // After the network and the rest of the machine are up, so an early
        // start does not race the services Nexus talks to.
        service.set_delayed_auto_start(true)?;
        // If nexus.exe itself dies, Windows starts it again. Its children are
        // already looked after by nexus.exe; this covers nexus.exe.
        service.update_failure_actions(ServiceFailureActions {
            reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(24 * 60 * 60)),
            reboot_msg: None,
            command: None,
            actions: Some(vec![
                ServiceAction {
                    action_type: ServiceActionType::Restart,
                    delay: Duration::from_secs(60),
                };
                3
            ]),
        })?;

        // The folder is locked down before the secret is written into it, so
        // there is no moment where another account could read it.
        acl::protect(&config_dir)?;
        config_file::write(&config_file, &settings)?;
        acl::protect(&config_dir)?;
        Ok(())
    })();
    if let Err(err) = configured {
        let _ = service.delete();
        return Err(err.context("installation failed and was undone"));
    }

    let logs = config_dir.join("logs");
    println!("Installed the Nexus service, running as {SERVICE_ACCOUNT}.");
    println!("  settings  {}", config_file.display());
    println!("  data      {}", config.data_dir.display());
    println!("  logs      {}", logs.display());

    if args.no_start {
        println!("Start it with: Start-Service Nexus");
        return Ok(());
    }

    service.start(&[] as &[&OsStr]).map_err(explain)?;
    println!("Starting. The first start creates the database and takes a minute or so.");
    wait_until_ready(&service, &config, &logs).await?;
    println!(
        "Nexus is running: {} (open {})",
        config.listen, config.app_url
    );
    Ok(())
}

/// Folds the command line into the settings. Kept separate so the rules can
/// be tested without a service manager.
pub fn apply_arguments(settings: &mut Settings, args: &InstallArgs) -> anyhow::Result<()> {
    match (&args.url, settings.contains_key("APP_URL")) {
        (Some(url), _) => {
            settings.insert("APP_URL".into(), url.trim_end_matches('/').to_string());
        }
        (None, true) => {}
        (None, false) => bail!(
            "--url is required: the address people will type, e.g. --url https://nexus.example.com"
        ),
    }
    if let Some(listen) = args.listen {
        settings.insert("LISTEN_ADDR".into(), listen.to_string());
    }
    if let Some(url) = &args.database_url {
        settings.insert("DATABASE_URL".into(), url.clone());
    }
    if !settings.contains_key("AUTH_SECRET") {
        let mut bytes = [0u8; 32];
        SysRng
            .try_fill_bytes(&mut bytes)
            .context("the operating system's random number generator is unavailable")?;
        settings.insert("AUTH_SECRET".into(), hex::encode(bytes));
    }
    Ok(())
}

pub fn service_info(exe: &Path, config_file: &Path) -> ServiceInfo {
    ServiceInfo {
        name: SERVICE_NAME.into(),
        display_name: "Nexus".into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe.to_path_buf(),
        launch_arguments: vec![
            OsString::from("service"),
            OsString::from("--config"),
            config_file.as_os_str().to_owned(),
        ],
        dependencies: Vec::new(),
        account_name: Some(SERVICE_ACCOUNT.into()),
        account_password: None,
    }
}

async fn wait_until_ready(
    service: &windows_service::service::Service,
    config: &Config,
    logs: &Path,
) -> anyhow::Result<()> {
    let front = format!("http://{}", config.listen);
    let client = crate::front_door::http_client();
    let started = Instant::now();
    let limit = Duration::from_secs(5 * 60);

    loop {
        let status = service.query_status()?;
        if status.current_state == ServiceState::Stopped {
            bail!(
                "the service stopped while starting. The reason is in the newest file in {}",
                logs.display()
            );
        }
        if answers(&client, &format!("{front}/api/v1/health/ready")).await
            && answers(&client, &format!("{front}/api/auth/jwks")).await
        {
            return Ok(());
        }
        if started.elapsed() > limit {
            bail!(
                "the service is running but Nexus did not answer within five minutes. See {}",
                logs.display()
            );
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn answers(client: &crate::front_door::HttpClient, url: &str) -> bool {
    let Ok(uri) = url.parse() else { return false };
    let mut request = axum::extract::Request::new(axum::body::Body::empty());
    *request.uri_mut() = uri;
    matches!(
        tokio::time::timeout(Duration::from_secs(5), client.request(request)).await,
        Ok(Ok(response)) if response.status().is_success()
    )
}

pub fn uninstall() -> anyhow::Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(explain)?;
    let service = match manager.open_service(
        SERVICE_NAME,
        ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
    ) {
        Ok(service) => service,
        Err(windows_service::Error::Winapi(err))
            if err.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST) =>
        {
            println!("Nexus is not installed.");
            return Ok(());
        }
        Err(err) => return Err(explain(err)),
    };

    if service.query_status()?.current_state != ServiceState::Stopped {
        println!("Stopping Nexus. PostgreSQL finishes its checkpoint first; allow up to a minute.");
        service.stop().map_err(explain)?;
        let started = Instant::now();
        while service.query_status()?.current_state != ServiceState::Stopped {
            if started.elapsed() > Duration::from_secs(180) {
                bail!("Nexus did not stop within three minutes; it has not been removed");
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    service.delete().map_err(explain)?;
    println!("Removed the Nexus service.");
    println!(
        "Settings, data and logs are kept (by default in {}). Delete that folder to remove them too.",
        config::default_config_file()
            .ok()
            .and_then(|f| f.parent().map(Path::to_path_buf))
            .map(|d| d.display().to_string())
            .unwrap_or_else(|| r"%ProgramData%\Nexus".to_string())
    );
    Ok(())
}

/// Turns the service manager's "access denied" into the one thing to do about it.
fn explain(err: windows_service::Error) -> anyhow::Error {
    match &err {
        windows_service::Error::Winapi(io) if io.raw_os_error() == Some(ERROR_ACCESS_DENIED) => {
            anyhow::anyhow!(
                "Windows refused: installing or removing a service needs an elevated console. \
                 Right-click PowerShell, choose \"Run as administrator\", and run this again."
            )
        }
        _ => anyhow::Error::new(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(url: Option<&str>) -> InstallArgs {
        InstallArgs {
            url: url.map(str::to_string),
            listen: None,
            node: None,
            database_url: None,
            config: None,
            no_start: false,
        }
    }

    #[test]
    fn first_install_needs_a_url_and_gets_a_new_secret() {
        let mut settings = Settings::new();
        assert!(apply_arguments(&mut settings, &args(None)).is_err());

        apply_arguments(
            &mut settings,
            &args(Some("https://nexus.brandkontoret.se/")),
        )
        .unwrap();

        assert_eq!(settings["APP_URL"], "https://nexus.brandkontoret.se");
        assert_eq!(settings["AUTH_SECRET"].len(), 64);
    }

    #[test]
    fn a_reinstall_keeps_the_secret_and_the_url() {
        let mut settings = Settings::from([
            (
                "APP_URL".to_string(),
                "https://nexus.brandkontoret.se".to_string(),
            ),
            ("AUTH_SECRET".to_string(), "keep-me".to_string()),
            ("POSTGRES_ADDR".to_string(), "127.0.0.1:25432".to_string()),
        ]);
        let mut again = args(None);
        again.listen = Some("127.0.0.1:8088".parse().unwrap());

        apply_arguments(&mut settings, &again).unwrap();

        assert_eq!(settings["AUTH_SECRET"], "keep-me");
        assert_eq!(settings["APP_URL"], "https://nexus.brandkontoret.se");
        assert_eq!(settings["LISTEN_ADDR"], "127.0.0.1:8088");
        // Settings someone added by hand survive too.
        assert_eq!(settings["POSTGRES_ADDR"], "127.0.0.1:25432");
    }

    #[test]
    fn the_service_runs_as_its_virtual_account_with_its_config() {
        let info = service_info(
            Path::new(r"C:\Program Files\Nexus\nexus.exe"),
            Path::new(r"C:\ProgramData\Nexus\nexus.env"),
        );

        assert_eq!(
            info.account_name.as_deref(),
            Some(OsStr::new(r"NT SERVICE\Nexus"))
        );
        assert!(info.account_password.is_none());
        assert_eq!(
            info.launch_arguments,
            ["service", "--config", r"C:\ProgramData\Nexus\nexus.env"].map(OsString::from)
        );
        assert_eq!(info.start_type, ServiceStartType::AutoStart);
    }
}

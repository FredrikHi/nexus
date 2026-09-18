//! Everything that exists only because Nexus runs as a Windows service.

pub mod acl;
pub mod install;
pub mod paths;
pub mod service;

/// The name `sc.exe`, `Get-Service` and the Services console know it by.
pub const SERVICE_NAME: &str = "Nexus";

/// A virtual account: created by Windows with the service, no password to
/// manage, and none of an administrator's rights. PostgreSQL refuses to run
/// with those, and a telemetry service has no business holding them anyway.
pub const SERVICE_ACCOUNT: &str = r"NT SERVICE\Nexus";

//! Makes child processes die with this one.
//!
//! On a clean stop the supervisor ends its children itself. What this covers
//! is the other case: nexus.exe killed from Task Manager, or crashing. Without
//! it the API and the auth service keep running, orphaned, holding their
//! ports, and the next start fails because those ports are taken.

use anyhow::Context;
use tokio::process::Child;

#[cfg(windows)]
pub struct KillOnClose {
    job: win32job::Job,
}

#[cfg(windows)]
impl KillOnClose {
    /// A Windows job object with "kill on close" set. Windows closes the job's
    /// handle when this process ends, however it ends, and every process in the
    /// job goes with it. The operating system enforces this, not our code, so it
    /// holds even when our code never gets to run again.
    pub fn new() -> anyhow::Result<Self> {
        let mut info = win32job::ExtendedLimitInfo::new();
        info.limit_kill_on_job_close();
        let job = win32job::Job::create_with_limit_info(&info)
            .context("could not create a Windows job object")?;
        Ok(Self { job })
    }

    pub fn adopt(&self, child: &Child) -> anyhow::Result<()> {
        let handle = child
            .raw_handle()
            .context("the process exited before it could be adopted")?;
        self.job
            .assign_process(handle as isize)
            .context("could not add the process to the job object")?;
        Ok(())
    }
}

/// Elsewhere this is a no-op: nexus.exe is built for Windows, and on Linux the
/// Docker deployment already gives each service its own lifetime.
#[cfg(not(windows))]
pub struct KillOnClose;

#[cfg(not(windows))]
impl KillOnClose {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self)
    }

    pub fn adopt(&self, _child: &Child) -> anyhow::Result<()> {
        Ok(())
    }
}

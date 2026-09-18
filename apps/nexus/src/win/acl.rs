//! Who may read the folder holding the config file, the database and the
//! password to it.
//!
//! `%ProgramData%` lets every user of the machine read what is created inside
//! it. That is the wrong default for a folder with AUTH_SECRET in it, so the
//! folder stops inheriting and allows exactly three: the operating system,
//! administrators, and the service's own account.
//!
//! Done with `icacls`, which every Windows has, rather than the Win32 security
//! API: a handful of `unsafe` calls to build a descriptor, for what one
//! well-known command line does.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context};

use super::SERVICE_ACCOUNT;

/// By SID, not by name: on a Swedish Windows the group is "Administratörer",
/// and a name that does not resolve fails the whole command.
const SYSTEM: &str = "*S-1-5-18";
const ADMINISTRATORS: &str = "*S-1-5-32-544";

/// Removes inherited entries from `dir` and grants full control to the three
/// accounts above, inherited by everything created inside it from now on.
pub fn protect_args(dir: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![dir.into(), "/inheritance:r".into(), "/grant:r".into()];
    for account in [SYSTEM, ADMINISTRATORS, SERVICE_ACCOUNT] {
        // (OI)(CI): applies to files and folders inside, not only this folder.
        args.push(format!("{account}:(OI)(CI)F").into());
    }
    args
}

/// Makes everything already inside `dir` inherit from it again, replacing
/// whatever each file had before. A reinstall over an older, readable folder
/// is otherwise protected only for new files.
pub fn reset_children_args(dir: &Path) -> Vec<OsString> {
    vec![
        dir.join("*").into(),
        "/reset".into(),
        "/T".into(),
        "/C".into(),
        "/Q".into(),
    ]
}

pub fn protect(dir: &Path) -> anyhow::Result<()> {
    icacls(&protect_args(dir))?;
    let has_children = std::fs::read_dir(dir)
        .with_context(|| format!("cannot list {}", dir.display()))?
        .next()
        .is_some();
    if has_children {
        icacls(&reset_children_args(dir))?;
    }
    Ok(())
}

fn icacls(args: &[OsString]) -> anyhow::Result<()> {
    let output = Command::new("icacls")
        .args(args)
        .output()
        .context("could not run icacls")?;
    if !output.status.success() {
        // icacls reports its errors on stdout.
        bail!(
            "icacls {} failed: {} {}",
            args.iter()
                .map(|a| a.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" "),
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: Vec<OsString>) -> Vec<String> {
        args.into_iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn the_folder_stops_inheriting_and_allows_exactly_three_accounts() {
        let args = strings(protect_args(Path::new(r"C:\ProgramData\Nexus")));

        assert_eq!(
            args,
            [
                r"C:\ProgramData\Nexus",
                "/inheritance:r",
                "/grant:r",
                "*S-1-5-18:(OI)(CI)F",
                "*S-1-5-32-544:(OI)(CI)F",
                r"NT SERVICE\Nexus:(OI)(CI)F",
            ]
        );
    }

    #[test]
    fn existing_files_are_reset_to_inherit_from_the_folder() {
        let args = strings(reset_children_args(Path::new(r"C:\ProgramData\Nexus")));
        assert_eq!(
            args,
            [r"C:\ProgramData\Nexus\*", "/reset", "/T", "/C", "/Q"]
        );
    }

    /// Runs icacls for real on a temporary folder, with this account in place
    /// of the service's, which does not exist on a machine without Nexus
    /// installed. Proves the arguments are ones icacls accepts, and that a
    /// file created before protecting ends up without the Users group.
    #[test]
    fn icacls_accepts_the_arguments_and_existing_files_lose_users_access() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("nexus.env"), "AUTH_SECRET='x'").unwrap();
        let me = format!(
            "{}\\{}",
            std::env::var("USERDOMAIN").unwrap(),
            std::env::var("USERNAME").unwrap()
        );

        let mut args = protect_args(dir.path());
        args.pop();
        args.push(format!("{me}:(OI)(CI)F").into());
        icacls(&args).unwrap();
        icacls(&reset_children_args(dir.path())).unwrap();

        let listing = Command::new("icacls")
            .arg(dir.path().join("nexus.env"))
            .output()
            .unwrap();
        let listing = String::from_utf8_lossy(&listing.stdout);
        // Built-in Users is S-1-5-32-545; its localized name varies, so check
        // the entries that are there instead: exactly the three granted.
        let entries = listing.lines().filter(|l| l.contains(":(I)")).count();
        assert_eq!(entries, 3, "{listing}");
        assert!(listing.contains(&me), "{listing}");
    }
}

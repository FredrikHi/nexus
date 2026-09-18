//! Paths the service account can and cannot use.

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context};

/// `std::fs::canonicalize` on Windows returns `\\?\C:\...`. Correct, but it
/// reads as a mistake in a config file or an error message, and some programs
/// cannot open such a path. Local drive paths lose the prefix; network paths
/// (`\\?\UNC\...`) keep it, since they need it.
pub fn plain(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path,
    }
}

/// The real location of `path`, following symlinks, without the `\\?\`.
pub fn resolve(path: &Path) -> anyhow::Result<PathBuf> {
    let real = std::fs::canonicalize(path)
        .with_context(|| format!("{} does not exist", path.display()))?;
    Ok(plain(real))
}

/// Whether `path` is inside `root`, comparing the way Windows does: without
/// regard to case.
pub fn is_within(path: &Path, root: &Path) -> bool {
    let normal = |p: &Path| -> Vec<String> {
        p.components()
            .filter(|c| !matches!(c, Component::CurDir))
            .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
            .collect()
    };
    let (path, root) = (normal(path), normal(root));
    path.len() >= root.len() && path[..root.len()] == root[..]
}

/// Refuses a path inside someone's user profile, other than the shared Public
/// one.
///
/// The service runs as its own account, which cannot read another account's
/// profile. A path there works when tried from the installing admin's console
/// and fails the moment the service starts, with an access-denied error that
/// names neither the path nor the reason. nvm-windows is the usual way to end
/// up here: `C:\nvm4w\nodejs` looks machine-wide and links into a profile.
pub fn refuse_user_profile(what: &str, path: &Path, instead: &str) -> anyhow::Result<()> {
    let Some(profile) = std::env::var_os("USERPROFILE") else {
        return Ok(());
    };
    let Some(profiles) = Path::new(&profile).parent() else {
        return Ok(());
    };
    let profiles = resolve(profiles).unwrap_or_else(|_| profiles.to_path_buf());
    let public = profiles.join("Public");

    if is_within(path, &profiles) && !is_within(path, &public) {
        bail!(
            "{what} is at {}, inside a user profile. The Nexus service runs as its own \
             account and cannot read files there. {instead}",
            path.display()
        );
    }
    Ok(())
}

/// The first `name` in a PATH folder.
pub fn find_on_path(name: impl AsRef<OsStr>) -> Option<PathBuf> {
    let name = name.as_ref();
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_paths_lose_the_verbatim_prefix_network_paths_keep_it() {
        assert_eq!(
            plain(PathBuf::from(r"\\?\C:\Program Files\nodejs\node.exe")),
            PathBuf::from(r"C:\Program Files\nodejs\node.exe")
        );
        assert_eq!(
            plain(PathBuf::from(r"\\?\UNC\server\share\node.exe")),
            PathBuf::from(r"\\?\UNC\server\share\node.exe")
        );
        assert_eq!(
            plain(PathBuf::from(r"D:\Nexus")),
            PathBuf::from(r"D:\Nexus")
        );
    }

    #[test]
    fn within_ignores_case_and_needs_whole_folder_names() {
        let users = Path::new(r"C:\Users");

        assert!(is_within(
            Path::new(r"c:\users\fredrik\AppData\Local\nvm\node.exe"),
            users
        ));
        assert!(is_within(Path::new(r"C:\Users"), users));
        // A sibling whose name merely starts the same is not inside.
        assert!(!is_within(Path::new(r"C:\UsersBackup\node.exe"), users));
        assert!(!is_within(
            Path::new(r"C:\Program Files\nodejs\node.exe"),
            users
        ));
    }

    #[test]
    fn a_path_in_this_accounts_profile_is_refused_with_the_way_out() {
        let profile = PathBuf::from(std::env::var_os("USERPROFILE").expect("USERPROFILE"));
        let inside = profile.join(r"AppData\Local\nvm\v22.22.0\node.exe");

        let err = refuse_user_profile("Node.js", &inside, "Pass --node.").unwrap_err();

        assert!(err.to_string().contains("cannot read files there"), "{err}");
        assert!(err.to_string().contains("Pass --node."), "{err}");
    }

    #[test]
    fn paths_outside_profiles_and_the_public_profile_are_accepted() {
        let profile = PathBuf::from(std::env::var_os("USERPROFILE").expect("USERPROFILE"));
        let public = profile.parent().unwrap().join(r"Public\node.exe");

        refuse_user_profile(
            "Node.js",
            Path::new(r"C:\Program Files\nodejs\node.exe"),
            "",
        )
        .unwrap();
        refuse_user_profile("Node.js", &public, "").unwrap();
    }
}

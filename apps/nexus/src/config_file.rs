//! `nexus.env`: the settings a Windows service reads instead of its environment.
//!
//! `KEY=value` lines under the same names as the environment variables, so
//! everything documented for one works in the other. Values are written in
//! single quotes, which dotenv treats literally: a Windows path's backslashes
//! and spaces survive the round trip without escaping rules to get wrong.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context};

/// Every setting in the file. Ordered, so a rewritten file keeps its keys in a
/// stable order and a diff of two versions shows only what changed.
pub type Settings = BTreeMap<String, String>;

pub fn read(path: &Path) -> anyhow::Result<Settings> {
    let entries =
        dotenvy::from_path_iter(path).with_context(|| format!("cannot read {}", path.display()))?;
    let mut settings = Settings::new();
    for entry in entries {
        let (key, value) = entry
            .with_context(|| format!("{} has a line that is not KEY=value", path.display()))?;
        settings.insert(key, value);
    }
    Ok(settings)
}

/// Writes the whole file anew. Written to a temporary file first and then
/// renamed over the old one, so a crash halfway leaves the previous file
/// intact rather than a truncated one missing AUTH_SECRET.
pub fn write(path: &Path, settings: &Settings) -> anyhow::Result<()> {
    let mut text = String::from(
        "# Nexus settings, read by the Windows service on every start.\n\
         # Same names as the environment variables; see the README.\n\
         # AUTH_SECRET signs sessions: changing it signs everyone out.\n\n",
    );
    for (key, value) in settings {
        if value.contains('\'') || value.contains('\n') || value.contains('\r') {
            bail!("{key} contains a quote or a line break, which nexus.env cannot hold");
        }
        text.push_str(&format!("{key}='{value}'\n"));
    }

    let temporary = path.with_extension("env.tmp");
    std::fs::write(&temporary, text)
        .with_context(|| format!("cannot write {}", temporary.display()))?;
    // On Windows, rename replaces an existing destination.
    std::fs::rename(&temporary, path)
        .with_context(|| format!("cannot replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_paths_and_urls_survive_a_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nexus.env");
        let settings = Settings::from([
            (
                "NODE_EXE".to_string(),
                r"C:\Program Files\nodejs\node.exe".to_string(),
            ),
            (
                "APP_URL".to_string(),
                "https://nexus.brandkontoret.se".to_string(),
            ),
            (
                "DATABASE_URL".to_string(),
                "postgres://u:p%40ss=1&x@db:5432/n?sslmode=require".to_string(),
            ),
            (
                "AUTH_SECRET".to_string(),
                "a1b2#not-a-comment$HOME".to_string(),
            ),
        ]);

        write(&path, &settings).unwrap();

        assert_eq!(read(&path).unwrap(), settings);
    }

    #[test]
    fn a_rewrite_replaces_the_file_and_leaves_no_temporary_behind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nexus.env");
        write(&path, &Settings::from([("A".to_string(), "1".to_string())])).unwrap();

        write(&path, &Settings::from([("A".to_string(), "2".to_string())])).unwrap();

        assert_eq!(read(&path).unwrap()["A"], "2");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn a_value_that_cannot_be_quoted_is_refused_rather_than_mangled() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::from([("APP_URL".to_string(), "it's".to_string())]);

        let err = write(&dir.path().join("nexus.env"), &settings).unwrap_err();

        assert!(err.to_string().contains("APP_URL"), "{err}");
    }

    #[test]
    fn hand_written_files_without_quotes_are_read_too() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nexus.env");
        std::fs::write(
            &path,
            "# a comment\nAPP_URL=https://nexus.example.com\n\nLISTEN_ADDR=127.0.0.1:8080\n",
        )
        .unwrap();

        let settings = read(&path).unwrap();

        assert_eq!(settings["APP_URL"], "https://nexus.example.com");
        assert_eq!(settings["LISTEN_ADDR"], "127.0.0.1:8080");
    }
}

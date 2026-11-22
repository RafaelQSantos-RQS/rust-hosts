//! Backups do arquivo de hosts antes de cada aplicação.

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use time::OffsetDateTime;
use time::macros::format_description;

use crate::Error;

/// Quantos backups ficam no disco (design.md, "Aplicar", passo 4).
pub const KEEP: usize = 20;

const STAMP: &[time::format_description::FormatItem<'static>] =
    format_description!("[year][month][day]T[hour][minute][second]Z");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    pub path: PathBuf,
    pub size: u64,
    /// Ordena e rotula a lista do Restore.
    pub modified: OffsetDateTime,
}

pub fn default_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join(".local/state/rust-hosts/backups")
}

/// Grava um backup novo e descarta os mais antigos além de [`KEEP`].
pub fn create(dir: &Path, contents: &[u8]) -> Result<PathBuf, Error> {
    fs::create_dir_all(dir)?;

    let stamp = OffsetDateTime::now_utc().format(STAMP)?;
    let path = unused_path(dir, &stamp);

    // temp + rename: um backup truncado por queda de energia é pior que nenhum.
    let temp = dir.join(format!("hosts-{stamp}.tmp"));
    let mut file = File::create(&temp)?;
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temp, &path)?;

    prune(dir, KEEP)?;
    Ok(path)
}

/// Do mais novo para o mais antigo; diretório ausente é uma lista vazia.
pub fn list(dir: &Path) -> Result<Vec<Backup>, Error> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };

    let mut backups = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.extension() != Some(OsStr::new("bak")) {
            continue;
        }
        let metadata = fs::metadata(&path)?;
        backups.push(Backup {
            path,
            size: metadata.len(),
            modified: metadata.modified()?.into(),
        });
    }

    backups.sort_by_key(|backup| std::cmp::Reverse(backup.modified));
    Ok(backups)
}

pub fn prune(dir: &Path, keep: usize) -> Result<(), Error> {
    for backup in list(dir)?.into_iter().skip(keep) {
        match fs::remove_file(&backup.path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Dois Apply no mesmo segundo não podem sobrescrever o backup anterior.
fn unused_path(dir: &Path, stamp: &str) -> PathBuf {
    let mut path = dir.join(format!("hosts-{stamp}.bak"));
    let mut counter = 1;
    while path.exists() {
        path = dir.join(format!("hosts-{stamp}-{counter}.bak"));
        counter += 1;
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_writes_contents_and_keeps_same_second_backups() {
        let dir = tempfile::tempdir().unwrap();

        let first = create(dir.path(), b"one\n").unwrap();
        let second = create(dir.path(), b"two\n").unwrap();

        assert_ne!(first, second);
        assert_eq!(fs::read(&first).unwrap(), b"one\n");
        assert_eq!(fs::read(&second).unwrap(), b"two\n");
    }

    #[test]
    fn list_is_empty_without_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list(&dir.path().join("missing")).unwrap().is_empty());
    }

    #[test]
    fn list_returns_newest_first_and_ignores_other_files() {
        let dir = tempfile::tempdir().unwrap();
        let oldest = create(dir.path(), b"old\n").unwrap();
        let newest = create(dir.path(), b"new\n").unwrap();
        fs::write(dir.path().join("notes.txt"), b"x").unwrap();

        let backups = list(dir.path()).unwrap();

        assert_eq!(backups.len(), 2);
        let paths: Vec<_> = backups.iter().map(|backup| backup.path.clone()).collect();
        assert!(paths.contains(&oldest) && paths.contains(&newest));
        assert_eq!(backups[0].size, 4);
    }

    #[test]
    fn prune_keeps_only_the_newest() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..5 {
            create(dir.path(), format!("{index}\n").as_bytes()).unwrap();
        }

        prune(dir.path(), 2).unwrap();

        assert_eq!(list(dir.path()).unwrap().len(), 2);
    }
}

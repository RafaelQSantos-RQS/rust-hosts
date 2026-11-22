//! Leitura, hash e escrita do arquivo de hosts.

use std::fs::{self, File, OpenOptions, Permissions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::Error;
use crate::block::Document;

/// Teto de sanidade: o /etc/hosts real tem poucos KB.
pub const MAX_BYTES: usize = 1 << 20;

pub fn read(path: &Path) -> Result<String, Error> {
    let bytes = fs::read(path)?;
    String::from_utf8(bytes).map_err(|_| Error::NotUtf8)
}

pub fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Troca só o bloco gerenciado; o resto do arquivo fica byte a byte.
pub fn replace_block(current: &str, incoming: &str) -> Result<String, Error> {
    let incoming = Document::parse(incoming)?;
    let Some(block) = incoming.block else {
        return Err(Error::MalformedBlock);
    };
    // O helper recebe só o bloco: qualquer coisa fora dos marcadores é rejeitada.
    if !incoming.before.is_empty() || !incoming.after.trim().is_empty() {
        return Err(Error::MalformedBlock);
    }

    let mut document = Document::parse(current)?;
    document.block = Some(block);
    Ok(document.render())
}

/// temp + fsync + rename, preservando dono e modo do original.
// shortcut: não copia o rótulo SELinux; em sistema enforcing pode exigir `restorecon /etc/hosts`. Upgrade: getxattr/setxattr de security.selinux.
pub fn write_atomic(path: &Path, contents: &str) -> Result<(), Error> {
    let metadata = fs::metadata(path)?;
    let temp = path.with_extension("rust-hosts-tmp");

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temp)?;

    // O temporário nasce de root; sem reaplicar o dono, o /etc/hosts muda de dono.
    // SAFETY: o fd é válido e fchown só falha com EPERM/EBADF, ambos reportados abaixo.
    let chowned = unsafe { libc::fchown(file.as_raw_fd(), metadata.uid(), metadata.gid()) };
    if chowned != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    file.set_permissions(Permissions::from_mode(metadata.mode() & 0o7777))?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    drop(file);

    fs::rename(&temp, path)?;

    // Durabilidade do rename, não só do conteúdo.
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CURRENT: &str = "127.0.0.1 localhost\n\
# BEGIN rust-hosts\n\
# Old\n\
10.0.0.1 old.test\n\
# END rust-hosts\n\
10.0.0.2 tail\n";

    #[test]
    fn hash_matches_known_vector() {
        assert_eq!(
            hash(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn replace_block_rewrites_only_the_block() {
        let incoming = "# BEGIN rust-hosts\n# New\n# 10.0.0.3 new.test\n# END rust-hosts\n";

        let result = replace_block(CURRENT, incoming).unwrap();

        assert_eq!(
            result,
            "127.0.0.1 localhost\n\
# BEGIN rust-hosts\n\
# New\n\
# 10.0.0.3 new.test\n\
# END rust-hosts\n\
10.0.0.2 tail\n"
        );
    }

    #[test]
    fn replace_block_appends_when_missing() {
        let incoming = "# BEGIN rust-hosts\n# Dev\n# END rust-hosts\n";

        let result = replace_block("127.0.0.1 localhost", incoming).unwrap();

        assert_eq!(
            result,
            "127.0.0.1 localhost\n# BEGIN rust-hosts\n# Dev\n# END rust-hosts\n"
        );
    }

    #[test]
    fn replace_block_rejects_content_outside_the_block() {
        let incoming = "junk\n# BEGIN rust-hosts\n# Dev\n# END rust-hosts\n";

        assert!(matches!(
            replace_block(CURRENT, incoming),
            Err(Error::MalformedBlock)
        ));
    }

    #[test]
    fn write_atomic_preserves_mode_and_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "old\n").unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o640)).unwrap();

        write_atomic(&path, "new\n").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "new\n");
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o640);
        assert!(!dir.path().join("hosts.rust-hosts-tmp").exists());
    }
}

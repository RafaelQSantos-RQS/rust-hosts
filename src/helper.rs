//! Lógica do helper root-owned: confere o hash, valida e grava o arquivo de hosts.

use std::path::Path;

use crate::block::Document;
use crate::{Error, hosts};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    ApplyBlock,
    Restore,
}

/// Contrato de argv/stdin/exit codes. Só muda quando a GUI e o helper deixam de
/// se entender; o helper instalado e o app novo precisam concordar nisso.
pub const PROTOCOL: u32 = 1;

/// `input` é o conteúdo recebido no stdin; `hosts_path` é constante no binário.
pub fn run(mode: Mode, expected_hash: &str, input: &str, hosts_path: &Path) -> Result<(), Error> {
    if input.len() > hosts::MAX_BYTES {
        return Err(Error::TooLarge(hosts::MAX_BYTES));
    }

    let current = hosts::read(hosts_path)?;
    if hosts::hash(current.as_bytes()) != expected_hash {
        return Err(Error::HashMismatch);
    }

    let output = match mode {
        Mode::ApplyBlock => hosts::replace_block(&current, input)?,
        Mode::Restore => {
            // Restore é a única operação que troca o arquivo inteiro.
            Document::parse(input)?;
            input.to_owned()
        }
    };

    hosts::write_atomic(hosts_path, &output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture(content: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, content).unwrap();
        (dir, path)
    }

    #[test]
    fn run_rejects_a_stale_hash_and_keeps_the_file() {
        let (_dir, path) = fixture("127.0.0.1 localhost\n");

        let result = run(Mode::Restore, "00", "other\n", &path);

        assert!(matches!(result, Err(Error::HashMismatch)));
        assert_eq!(fs::read_to_string(&path).unwrap(), "127.0.0.1 localhost\n");
    }

    #[test]
    fn run_applies_a_block_over_the_current_file() {
        let current = "127.0.0.1 localhost\n";
        let (_dir, path) = fixture(current);
        let block = "# BEGIN rust-hosts\n# Dev\n127.0.0.1 dev.test\n# END rust-hosts\n";

        run(
            Mode::ApplyBlock,
            &hosts::hash(current.as_bytes()),
            block,
            &path,
        )
        .unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "127.0.0.1 localhost\n# BEGIN rust-hosts\n# Dev\n127.0.0.1 dev.test\n# END rust-hosts\n"
        );
    }

    #[test]
    fn run_restore_rewrites_the_whole_file() {
        let current = "127.0.0.1 localhost\n";
        let (_dir, path) = fixture(current);
        let backup = "# a backup with no block\n10.0.0.9 only.test\n";

        run(
            Mode::Restore,
            &hosts::hash(current.as_bytes()),
            backup,
            &path,
        )
        .unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), backup);
    }

    #[test]
    fn run_rejects_oversized_input() {
        let current = "";
        let (_dir, path) = fixture(current);
        let huge = "x".repeat(hosts::MAX_BYTES + 1);

        let result = run(
            Mode::Restore,
            &hosts::hash(current.as_bytes()),
            &huge,
            &path,
        );

        assert!(matches!(result, Err(Error::TooLarge(_))));
    }
}

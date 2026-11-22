//! Chamada do helper root-owned via pkexec (ADR-0002).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const HELPER: &str = "/usr/local/libexec/rust-hosts-helper";

/// O que o binário instalado respondeu quando o app abriu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelperState {
    Ready,
    /// Não existe, não é executável ou não respondeu.
    Missing,
    /// Respondeu, mas com outro protocolo: o par GUI/helper está descasado.
    Outdated,
}

/// Executa o helper instalado sem privilégio (só para ler a versão do contrato).
pub fn helper_state() -> HelperState {
    let Ok(output) = Command::new(HELPER).arg("--protocol").output() else {
        return HelperState::Missing;
    };
    if !output.status.success() {
        return HelperState::Outdated;
    }

    let reported = String::from_utf8_lossy(&output.stdout);
    if reported.trim() == crate::helper::PROTOCOL.to_string() {
        HelperState::Ready
    } else {
        HelperState::Outdated
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// `pkexec`: diálogo dispensado ou autorização negada.
    Cancelled,
    /// O helper recusou; `code` é o exit code dele.
    Helper { code: i32, message: String },
    /// Não foi possível executar o pkexec.
    Spawn(String),
}

pub fn apply_block(block: &str, expected_hash: &str) -> Result<(), Failure> {
    run("--apply-block", block, expected_hash)
}

/// Import e Restore mudam conteúdo fora do bloco, então reescrevem o arquivo inteiro.
pub fn write_full(content: &str, expected_hash: &str) -> Result<(), Failure> {
    run("--restore", content, expected_hash)
}

fn run(mode: &str, input: &str, expected_hash: &str) -> Result<(), Failure> {
    // Sem o helper instalado, o pkexec abriria um diálogo só para falhar depois.
    if !Path::new(HELPER).exists() {
        return Err(Failure::Spawn(format!(
            "{HELPER} is not installed; install it from the app"
        )));
    }

    let mut child = Command::new("pkexec")
        .arg(HELPER)
        .arg(mode)
        .arg("--expected-hash")
        .arg(expected_hash)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| Failure::Spawn(error.to_string()))?;

    {
        // O helper só escreve no stderr, então mandar o stdin inteiro antes de ler
        // a saída não trava em pipe cheio.
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| Failure::Spawn("stdin unavailable".to_owned()))?;
        stdin
            .write_all(input.as_bytes())
            .map_err(|error| Failure::Spawn(error.to_string()))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| Failure::Spawn(error.to_string()))?;

    checked(output)
}

/// O helper que acompanha este binário: mesma pasta (é assim no AppImage e no build).
pub fn helper_source() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let candidate = exe.parent()?.join("rust-hosts-helper");
    candidate.is_file().then_some(candidate)
}

/// Comando exato para quem preferir instalar na mão.
pub fn install_command(source: &Path) -> String {
    format!(
        "sudo install -Dm755 -o root -g root {} {HELPER}",
        source.display()
    )
}

/// Instala o helper root-owned via pkexec, sem shell: `install` recebe os args direto.
pub fn install_helper(source: &Path) -> Result<(), Failure> {
    let output = Command::new("pkexec")
        // Caminho absoluto: não depende do PATH que o pkexec repassa.
        .args(["/usr/bin/install", "-Dm755", "-o", "root", "-g", "root"])
        .arg(source)
        .arg(HELPER)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| Failure::Spawn(error.to_string()))?;

    checked(output)
}

fn checked(output: std::process::Output) -> Result<(), Failure> {
    if output.status.success() {
        return Ok(());
    }

    let code = output.status.code().unwrap_or(-1);
    // 126/127 são o pkexec recusando; qualquer outro código é o helper.
    if code == 126 || code == 127 {
        return Err(Failure::Cancelled);
    }
    Err(Failure::Helper {
        code,
        message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

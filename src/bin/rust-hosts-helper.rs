use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use rust_hosts::Error;
use rust_hosts::helper::{self, Mode};

/// Contrato com a GUI: o arquivo alvo é constante, o conteúdo vem no stdin.
const HOSTS: &str = "/etc/hosts";
const USAGE: &str =
    "usage: rust-hosts-helper --apply-block|--restore --expected-hash <hex> < content";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Pergunta de versão: responde sem privilégio e sem ler stdin.
    if args.len() == 1 && args[0] == "--protocol" {
        println!("{}", rust_hosts::helper::PROTOCOL);
        return ExitCode::SUCCESS;
    }

    let Some((mode, expected_hash)) = parse(&args) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };

    let mut input = String::new();
    let outcome = match std::io::stdin().read_to_string(&mut input) {
        Ok(_) => helper::run(mode, &expected_hash, &input, Path::new(HOSTS)),
        Err(error) => Err(Error::Io(error)),
    };

    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rust-hosts-helper: {error}");
            ExitCode::from(code(&error))
        }
    }
}

fn parse(args: &[String]) -> Option<(Mode, String)> {
    let mut mode = None;
    let mut hash = None;
    let mut args = args.iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--apply-block" | "--restore" => {
                let new = if arg == "--apply-block" {
                    Mode::ApplyBlock
                } else {
                    Mode::Restore
                };
                if mode.replace(new).is_some() {
                    return None;
                }
            }
            "--expected-hash" => {
                if hash.is_some() {
                    return None;
                }
                hash = Some(args.next()?.to_ascii_lowercase());
            }
            _ => return None,
        }
    }

    let hash = hash?;
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some((mode?, hash))
}

fn code(error: &Error) -> u8 {
    match error {
        Error::HashMismatch => 5,
        Error::Io(_) => 4,
        _ => 3,
    }
}

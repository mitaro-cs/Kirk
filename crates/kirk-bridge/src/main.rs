//! Мост строки состояния Claude Code.
//!
//! Claude Code запускает его после каждого ответа и передаёт JSON в stdin.
//! Мост сохраняет только лимиты (`kirk_core::claude::publish`), а если до
//! Kirk стояла своя строка состояния (`--then <base64>`), запускает её с тем
//! же входом – её вывод и есть строка состояния.

use std::io::{Read, Write};
use std::process::{Command, Stdio};

use kirk_core::claude;

fn main() {
    let mut input = Vec::new();
    if std::io::stdin().read_to_end(&mut input).is_err() {
        std::process::exit(1);
    }

    let now_s = kirk_core::now_ms() as f64 / 1000.0;
    if let Err(e) = claude::publish(&input, now_s, &claude::snapshot_path()) {
        // До первого ответа API лимитов нет – это не ошибка.
        if e != claude::BridgeError::NoRateLimits {
            eprintln!("Kirk: {e}");
        }
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    let previous = match args.as_slice() {
        [flag, encoded, ..] if flag == "--then" => {
            claude::previous_command(&format!("--then {encoded}"))
        }
        _ => None,
    };
    if let Some(cmd) = previous {
        std::process::exit(run_previous(&cmd, &input));
    }
}

/// Прежняя команда – строка оболочки: `sh -c` на Mac и Linux; на Windows –
/// `bash -c` (Git Bash, как у Claude Code), а без него – `cmd /C`.
fn run_previous(cmd: &str, input: &[u8]) -> i32 {
    let shells: &[(&str, &str)] = if cfg!(windows) {
        &[("bash", "-c"), ("cmd", "/C")]
    } else {
        &[("/bin/sh", "-c")]
    };
    for (shell, flag) in shells {
        let child = Command::new(shell)
            .arg(flag)
            .arg(cmd)
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn();
        match child {
            Ok(mut child) => {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(input);
                }
                return child.wait().ok().and_then(|s| s.code()).unwrap_or(1);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                eprintln!("Kirk: прежняя строка состояния: {e}");
                return 1;
            }
        }
    }
    1
}

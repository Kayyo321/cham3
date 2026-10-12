//! M2-001 provider probe executable.
//!
//! Modes (first argument):
//! * `report` — run all provider checks and print one line per passed check, plus the
//!   Argon2id oracle-comparison output as hex. Exit 0 only if every check passes.
//! * `panic` — deliberately panic. The deployed release profile must abort (SIGABRT);
//!   the dev profile unwinds (exit status 101). Used by tests/m2/fault/.
//! * `overflow <u64>` — add the argument to u64::MAX. Overflow checks must panic in
//!   every profile.
//! * `harden` — apply process hardening and report the resulting dumpable state.
#![forbid(unsafe_code)]

use std::process::ExitCode;

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().collect();
    match arguments.get(1).map(String::as_str) {
        Some("report") => match cham3_provider_probe::run_all() {
            Ok(passed) => {
                for check in passed {
                    println!("PASS {check}");
                }
                match cham3_provider_probe::argon2id_oracle_case() {
                    Ok(output) => {
                        println!("ARGON2ID_ORACLE_CASE {}", hex(&output));
                        ExitCode::SUCCESS
                    }
                    Err(failure) => {
                        println!("FAIL {}", failure.check);
                        ExitCode::FAILURE
                    }
                }
            }
            Err(failure) => {
                println!("FAIL {}", failure.check);
                ExitCode::FAILURE
            }
        },
        Some("panic") => {
            #[allow(clippy::panic)]
            {
                panic!("cham3 provider probe: deliberate panic");
            }
        }
        Some("overflow") => {
            let Some(value) = arguments.get(2).and_then(|text| text.parse::<u64>().ok()) else {
                eprintln!("usage: cham3_provider_probe overflow <u64>");
                return ExitCode::from(2);
            };
            #[allow(clippy::arithmetic_side_effects)]
            let sum = u64::MAX + value;
            println!("NO_OVERFLOW_TRAP {sum}");
            ExitCode::FAILURE
        }
        Some("harden") => match cham3_os_adapter::harden_process()
            .and_then(|()| cham3_os_adapter::is_dumpable())
        {
            Ok(dumpable) => {
                println!("DUMPABLE {dumpable}");
                if dumpable {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                }
            }
            Err(error) => {
                println!("FAIL {error}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("usage: cham3_provider_probe report|panic|overflow <u64>|harden");
            ExitCode::from(2)
        }
    }
}

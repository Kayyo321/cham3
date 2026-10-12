//! Differential probe for verification layers 1–5 (M2-003).
//!
//! Reads lines on stdin:
//!
//! * `key <kid_hex> <public_key_hex>` adds a resolver entry (synthetic test keys);
//! * `verify <kind> <envelope_hex>` verifies an object of that kind.
//!
//! and writes one line per `verify`: `ok <object_id_hex> <same|different>` (whether
//! the parsed payload re-encodes to the authenticated bytes) or
//! `err <layer> <code>`. Malformed probe input exits with status 2. The probe never
//! prints input bytes and is used only by tests/m2/lib/m2_harness.py.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufWriter, Write};
use std::process::ExitCode;

use cham3_contract_format::dcbor::encode;
use cham3_contract_format::hash::digest_t;
use cham3_contract_format::{key_resolver_t, object_kind_t, verify_object};

struct map_resolver_t {
    keys: BTreeMap<digest_t, [u8; 32]>,
}

impl key_resolver_t for map_resolver_t {
    fn resolve(&self, kid: &digest_t) -> Option<[u8; 32]> {
        self.keys.get(kid).copied()
    }
}

fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    text.as_bytes()
        .chunks(2)
        .map(|pair| match pair {
            [high, low] => {
                let digit = |byte: &u8| char::from(*byte).to_digit(16);
                let value = digit(high)?.checked_mul(16)?.checked_add(digit(low)?)?;
                u8::try_from(value).ok()
            }
            _ => None,
        })
        .collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    const Digits: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        for nibble in [byte >> 4, byte & 0x0f] {
            if let Some(digit) = Digits.get(usize::from(nibble)) {
                text.push(char::from(*digit));
            }
        }
    }
    text
}

fn handle(line: &str, resolver: &mut map_resolver_t) -> Option<Option<String>> {
    let mut parts = line.split(' ');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("key"), Some(kid), Some(public_key), None) => {
            let kid = digest_t::try_from(hex_decode(kid)?.as_slice()).ok()?;
            let public_key = <[u8; 32]>::try_from(hex_decode(public_key)?.as_slice()).ok()?;
            resolver.keys.insert(kid, public_key);
            Some(None)
        }
        (Some("verify"), Some(kind), Some(envelope), None) => {
            let kind = object_kind_t::from_name(kind)?;
            let envelope = hex_decode(envelope)?;
            let result = match verify_object(&envelope, kind, resolver) {
                Ok(verified) => {
                    let same = encode(&verified.payload.to_value()) == verified.payload_bytes;
                    format!(
                        "ok {} {}",
                        hex_encode(&verified.object_id),
                        if same { "same" } else { "different" }
                    )
                }
                Err(error) => format!("err {} {}", error.layer.as_str(), error.code.as_str()),
            };
            Some(Some(result))
        }
        _ => None,
    }
}

fn main() -> ExitCode {
    let mut resolver = map_resolver_t {
        keys: BTreeMap::new(),
    };
    let stdout = std::io::stdout();
    let mut output = BufWriter::new(stdout.lock());
    for (number, line) in std::io::stdin().lock().lines().enumerate() {
        let Ok(line) = line else {
            eprintln!("cham3_format_probe: unreadable input");
            return ExitCode::from(2);
        };
        match handle(&line, &mut resolver) {
            Some(Some(result)) => {
                if writeln!(output, "{result}").is_err() {
                    return ExitCode::from(2);
                }
            }
            Some(None) => {}
            None => {
                eprintln!(
                    "cham3_format_probe: malformed input line {}",
                    number.saturating_add(1)
                );
                return ExitCode::from(2);
            }
        }
    }
    if output.flush().is_err() {
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}

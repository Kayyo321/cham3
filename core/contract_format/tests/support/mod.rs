//! Test support: a minimal JSON reader for the published ECR v1 vectors, hex
//! helpers, the synthetic key resolver and a seeded generator.
//!
//! The vectors are produced by the independent oracle (tests/m2/oracle/ecr_oracle.py)
//! and contain only synthetic keys derived from public labels.

#![allow(dead_code)]

use std::collections::BTreeMap;

use cham3_contract_format::hash::digest_t;
use cham3_contract_format::key_resolver_t;

/// A parsed JSON value.
#[derive(Clone, Debug, PartialEq)]
pub enum json_t {
    null,
    boolean(bool),
    number(i64),
    text(String),
    array(Vec<json_t>),
    object(BTreeMap<String, json_t>),
}

impl json_t {
    pub fn get(&self, key: &str) -> Option<&json_t> {
        match self {
            json_t::object(members) => members.get(key),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            json_t::text(text) => Some(text),
            _ => None,
        }
    }

    pub fn items(&self) -> &[json_t] {
        match self {
            json_t::array(items) => items,
            _ => &[],
        }
    }

    pub fn as_number(&self) -> Option<i64> {
        match self {
            json_t::number(number) => Some(*number),
            _ => None,
        }
    }

    /// A required text member; panics with the member name otherwise (test code).
    pub fn field(&self, key: &str) -> &str {
        self.get(key)
            .and_then(json_t::as_text)
            .unwrap_or_else(|| panic!("vector member {key} missing or not text"))
    }
}

struct reader_t<'text> {
    bytes: &'text [u8],
    position: usize,
}

impl reader_t<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn next(&mut self) -> Result<u8, String> {
        let byte = self.peek().ok_or("unexpected end of JSON")?;
        self.position += 1;
        Ok(byte)
    }

    fn skip_space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.position += 1;
        }
    }

    fn expect(&mut self, literal: &str) -> Result<(), String> {
        for expected in literal.bytes() {
            if self.next()? != expected {
                return Err(format!("expected {literal} at byte {}", self.position));
            }
        }
        Ok(())
    }

    fn value(&mut self) -> Result<json_t, String> {
        self.skip_space();
        match self.peek().ok_or("unexpected end of JSON")? {
            b'{' => {
                self.position += 1;
                let mut members = BTreeMap::new();
                self.skip_space();
                if self.peek() == Some(b'}') {
                    self.position += 1;
                    return Ok(json_t::object(members));
                }
                loop {
                    self.skip_space();
                    let key = self.string()?;
                    self.skip_space();
                    self.expect(":")?;
                    let value = self.value()?;
                    if members.insert(key.clone(), value).is_some() {
                        return Err(format!("duplicate JSON member {key}"));
                    }
                    self.skip_space();
                    match self.next()? {
                        b',' => continue,
                        b'}' => return Ok(json_t::object(members)),
                        other => return Err(format!("unexpected {other} in object")),
                    }
                }
            }
            b'[' => {
                self.position += 1;
                let mut items = Vec::new();
                self.skip_space();
                if self.peek() == Some(b']') {
                    self.position += 1;
                    return Ok(json_t::array(items));
                }
                loop {
                    items.push(self.value()?);
                    self.skip_space();
                    match self.next()? {
                        b',' => continue,
                        b']' => return Ok(json_t::array(items)),
                        other => return Err(format!("unexpected {other} in array")),
                    }
                }
            }
            b'"' => Ok(json_t::text(self.string()?)),
            b't' => self.expect("true").map(|()| json_t::boolean(true)),
            b'f' => self.expect("false").map(|()| json_t::boolean(false)),
            b'n' => self.expect("null").map(|()| json_t::null),
            _ => {
                let start = self.position;
                while matches!(self.peek(), Some(b'-' | b'0'..=b'9')) {
                    self.position += 1;
                }
                let digits = std::str::from_utf8(&self.bytes[start..self.position])
                    .map_err(|error| error.to_string())?;
                digits
                    .parse()
                    .map(json_t::number)
                    .map_err(|_| format!("unsupported JSON number at byte {start}"))
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let mut value = 0u32;
        for _ in 0..4 {
            let digit = char::from(self.next()?)
                .to_digit(16)
                .ok_or("bad \\u escape")?;
            value = value * 16 + digit;
        }
        Ok(value)
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect("\"")?;
        let mut output = String::new();
        loop {
            let start = self.position;
            while !matches!(self.peek(), Some(b'"' | b'\\') | None) {
                self.position += 1;
            }
            output.push_str(
                std::str::from_utf8(&self.bytes[start..self.position])
                    .map_err(|error| error.to_string())?,
            );
            match self.next()? {
                b'"' => return Ok(output),
                _ => match self.next()? {
                    b'"' => output.push('"'),
                    b'\\' => output.push('\\'),
                    b'/' => output.push('/'),
                    b'b' => output.push('\u{8}'),
                    b'f' => output.push('\u{c}'),
                    b'n' => output.push('\n'),
                    b'r' => output.push('\r'),
                    b't' => output.push('\t'),
                    b'u' => {
                        let first = self.hex4()?;
                        let code = if (0xd800..0xdc00).contains(&first) {
                            self.expect("\\u")?;
                            let second = self.hex4()?;
                            0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00)
                        } else {
                            first
                        };
                        output.push(char::from_u32(code).ok_or("bad \\u code point")?);
                    }
                    other => return Err(format!("bad escape {other}")),
                },
            }
        }
    }
}

/// Parses a complete JSON document.
pub fn parse_json(text: &str) -> Result<json_t, String> {
    let mut reader = reader_t {
        bytes: text.as_bytes(),
        position: 0,
    };
    let value = reader.value()?;
    reader.skip_space();
    if reader.position != text.len() {
        return Err("trailing JSON content".to_owned());
    }
    Ok(value)
}

/// The published ECR v1 vector document.
pub fn vectors() -> json_t {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/m2/vectors/ecr_v1/vectors.json"
    );
    let text = std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path}: {error}"));
    parse_json(&text).unwrap_or_else(|error| panic!("parse {path}: {error}"))
}

pub fn hex_decode(text: &str) -> Vec<u8> {
    assert!(text.len() % 2 == 0, "odd hex length");
    (0..text.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&text[index..index + 2], 16)
                .unwrap_or_else(|_| panic!("bad hex at {index}"))
        })
        .collect()
}

pub fn hex_encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut text, byte| {
            text.push_str(&format!("{byte:02x}"));
            text
        })
}

pub fn fixed<const Length: usize>(text: &str) -> [u8; Length] {
    <[u8; Length]>::try_from(hex_decode(text).as_slice())
        .unwrap_or_else(|_| panic!("expected {Length} bytes"))
}

/// One synthetic vector key.
#[derive(Clone, Debug)]
pub struct synthetic_key_t {
    pub label: String,
    pub key_id: digest_t,
    pub public_key: [u8; 32],
    pub seed: [u8; 32],
}

pub fn synthetic_keys(document: &json_t) -> Vec<synthetic_key_t> {
    document
        .get("keys")
        .map(json_t::items)
        .unwrap_or_default()
        .iter()
        .map(|key| synthetic_key_t {
            label: key.field("label").to_owned(),
            key_id: fixed(key.field("key_id")),
            public_key: fixed(key.field("public_key")),
            seed: fixed(key.field("seed")),
        })
        .collect()
}

/// Resolver over the synthetic vector keys.
pub struct resolver_t {
    pub keys: BTreeMap<digest_t, [u8; 32]>,
}

impl resolver_t {
    pub fn new(keys: &[synthetic_key_t]) -> resolver_t {
        resolver_t {
            keys: keys
                .iter()
                .map(|key| (key.key_id, key.public_key))
                .collect(),
        }
    }
}

impl key_resolver_t for resolver_t {
    fn resolve(&self, kid: &digest_t) -> Option<[u8; 32]> {
        self.keys.get(kid).copied()
    }
}

/// Deterministic xorshift64* generator for seeded mutation tests.
pub struct generator_t {
    state: u64,
}

impl generator_t {
    pub fn new(seed: u64) -> generator_t {
        generator_t { state: seed | 1 }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        self.state.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Uniform-enough value in 0..bound (bound > 0).
    pub fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
}

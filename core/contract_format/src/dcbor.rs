//! Deterministic CBOR profile `cham3_dcbor_v1` (wire format §2): a strict decoder
//! with the §2.3 limits and the §2.4 first-error order, and the deterministic encoder.
//!
//! The decoder never allocates according to an untrusted length before that length
//! has been checked against the limits and the remaining input.

use crate::error::{error_code_t, format_error_t, format_result_t, layer_t};

/// Largest complete signed object, and largest byte string (wire format §2.3).
pub const MaxEncodedBytes: usize = 61_440;
/// Nesting depth of arrays, maps and tags in one decode.
pub const MaxDepth: usize = 8;
/// Items in one array or pairs in one map.
pub const MaxContainerItems: u64 = 256;
/// Text string length in bytes.
pub const MaxTextBytes: u64 = 1_024;
/// The COSE_Sign1 tag, allowed only as the outermost item of a signed object.
pub const SignedObjectTag: u64 = 18;

const MaxNegativeArgument: u64 = 0x7fff_ffff_ffff_ffff;
const MajorUnsigned: u8 = 0x00;
const MajorNegative: u8 = 0x20;
const MajorBytes: u8 = 0x40;
const MajorText: u8 = 0x60;
const MajorArray: u8 = 0x80;
const MajorMap: u8 = 0xa0;
const MajorTag: u8 = 0xc0;
const MajorSimple: u8 = 0xe0;
const MajorMask: u8 = 0xe0;
const AdditionalMask: u8 = 0x1f;
const EncodedFalse: u8 = 0xf4;
const EncodedTrue: u8 = 0xf5;

/// One decoded `cham3_dcbor_v1` data item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum value_t {
    unsigned(u64),
    /// The negative integer −1 − n, stored as n (n ≤ 2^63 − 1).
    negative(u64),
    bytes(Vec<u8>),
    text(String),
    array(Vec<value_t>),
    /// Key/value pairs; after a decode, in strictly ascending encoded-key order.
    map(Vec<(value_t, value_t)>),
    boolean(bool),
    tag(u64, Box<value_t>),
}

impl value_t {
    /// A text item.
    pub fn text_of(text: &str) -> value_t {
        value_t::text(text.to_owned())
    }

    /// A byte string item.
    pub fn bytes_of(bytes: &[u8]) -> value_t {
        value_t::bytes(bytes.to_vec())
    }

    /// A map item from (text key, value) pairs; the encoder orders the keys.
    pub fn text_map(pairs: Vec<(&str, value_t)>) -> value_t {
        value_t::map(
            pairs
                .into_iter()
                .map(|(key, value)| (value_t::text_of(key), value))
                .collect(),
        )
    }

    /// Looks up a text key of a map.
    pub fn get(&self, key: &str) -> Option<&value_t> {
        match self {
            value_t::map(pairs) => pairs.iter().find_map(|(candidate, value)| match candidate {
                value_t::text(text) if text == key => Some(value),
                _ => None,
            }),
            _ => None,
        }
    }
}

/// Decodes exactly one top-level item. `layer` is the layer reported on failure
/// (`envelope_decode` or `payload_decode`). Tag 18 is accepted only when
/// `allow_signed_object_tag` is set and only as the outermost item.
pub fn decode(
    data: &[u8],
    layer: layer_t,
    allow_signed_object_tag: bool,
) -> format_result_t<value_t> {
    let mut decoder = decoder_t {
        data,
        position: 0,
        layer,
    };
    if data.len() > MaxEncodedBytes {
        return decoder.fail(error_code_t::input_too_large);
    }
    let value = decoder.item(0, allow_signed_object_tag)?;
    if decoder.position != data.len() {
        return decoder.fail(error_code_t::trailing_bytes);
    }
    Ok(value)
}

struct decoder_t<'data> {
    data: &'data [u8],
    position: usize,
    layer: layer_t,
}

impl<'data> decoder_t<'data> {
    fn fail<ok_t>(&self, code: error_code_t) -> format_result_t<ok_t> {
        Err(format_error_t::new(self.layer, code))
    }

    fn error(&self, code: error_code_t) -> format_error_t {
        format_error_t::new(self.layer, code)
    }

    fn take(&mut self, count: u64) -> format_result_t<&'data [u8]> {
        let truncated = self.error(error_code_t::truncated);
        let count = usize::try_from(count).map_err(|_| truncated)?;
        let end = self.position.checked_add(count).ok_or(truncated)?;
        let chunk = self.data.get(self.position..end).ok_or(truncated)?;
        self.position = end;
        Ok(chunk)
    }

    fn take_array<const Count: usize>(&mut self) -> format_result_t<[u8; Count]> {
        let truncated = self.error(error_code_t::truncated);
        let count = u64::try_from(Count).map_err(|_| truncated)?;
        <[u8; Count]>::try_from(self.take(count)?).map_err(|_| truncated)
    }

    /// The argument of an item head; additional information 0–27 only.
    fn argument(&mut self, additional: u8) -> format_result_t<u64> {
        let (value, minimum) = match additional {
            0..=23 => return Ok(u64::from(additional)),
            24 => (u64::from(u8::from_be_bytes(self.take_array::<1>()?)), 24),
            25 => (
                u64::from(u16::from_be_bytes(self.take_array::<2>()?)),
                0x100,
            ),
            26 => (
                u64::from(u32::from_be_bytes(self.take_array::<4>()?)),
                0x1_0000,
            ),
            27 => (u64::from_be_bytes(self.take_array::<8>()?), 0x1_0000_0000),
            _ => return self.fail(error_code_t::reserved_additional_info),
        };
        if value < minimum {
            return self.fail(error_code_t::non_minimal_encoding);
        }
        Ok(value)
    }

    fn deeper(&self, depth: usize) -> format_result_t<usize> {
        match depth.checked_add(1) {
            Some(next) if next <= MaxDepth => Ok(next),
            _ => self.fail(error_code_t::depth_exceeded),
        }
    }

    fn item(&mut self, depth: usize, allow_root_tag: bool) -> format_result_t<value_t> {
        let [initial] = self.take_array::<1>()?;
        let major = initial & MajorMask;
        let additional = initial & AdditionalMask;
        if (28..=30).contains(&additional) {
            return self.fail(error_code_t::reserved_additional_info);
        }
        if additional == 31 {
            return self.fail(error_code_t::indefinite_length);
        }
        if major == MajorSimple {
            return match additional {
                20 => Ok(value_t::boolean(false)),
                21 => Ok(value_t::boolean(true)),
                25..=27 => {
                    let width = match additional {
                        25 => 2,
                        26 => 4,
                        _ => 8,
                    };
                    self.take(width)?;
                    self.fail(error_code_t::float_forbidden)
                }
                24 => {
                    self.take(1)?;
                    self.fail(error_code_t::simple_value_forbidden)
                }
                _ => self.fail(error_code_t::simple_value_forbidden),
            };
        }
        let argument = self.argument(additional)?;
        match major {
            MajorUnsigned => Ok(value_t::unsigned(argument)),
            MajorNegative => {
                if argument > MaxNegativeArgument {
                    return self.fail(error_code_t::integer_out_of_range);
                }
                Ok(value_t::negative(argument))
            }
            MajorBytes | MajorText => {
                if major == MajorText && argument > MaxTextBytes {
                    return self.fail(error_code_t::text_too_long);
                }
                if usize::try_from(argument).map_or(true, |length| length > MaxEncodedBytes) {
                    return self.fail(error_code_t::input_too_large);
                }
                let raw = self.take(argument)?;
                if major == MajorBytes {
                    return Ok(value_t::bytes(raw.to_vec()));
                }
                match std::str::from_utf8(raw) {
                    Ok(text) => Ok(value_t::text(text.to_owned())),
                    Err(_) => self.fail(error_code_t::invalid_utf8),
                }
            }
            MajorArray | MajorMap => {
                let inner = self.deeper(depth)?;
                if argument > MaxContainerItems {
                    return self.fail(error_code_t::container_too_large);
                }
                // Bounded by MaxContainerItems, so this conversion cannot fail in
                // practice; it fails closed if it ever did.
                let count = usize::try_from(argument)
                    .map_err(|_| self.error(error_code_t::container_too_large))?;
                if major == MajorArray {
                    let mut items = Vec::with_capacity(count);
                    for _ in 0..count {
                        items.push(self.item(inner, false)?);
                    }
                    return Ok(value_t::array(items));
                }
                self.map_items(inner, count)
            }
            MajorTag => {
                if !(allow_root_tag && depth == 0 && argument == SignedObjectTag) {
                    return self.fail(error_code_t::tag_forbidden);
                }
                let inner = self.deeper(depth)?;
                let content = self.item(inner, false)?;
                Ok(value_t::tag(argument, Box::new(content)))
            }
            _ => self.fail(error_code_t::reserved_additional_info),
        }
    }

    fn map_items(&mut self, inner: usize, count: usize) -> format_result_t<value_t> {
        let mut pairs = Vec::with_capacity(count);
        let mut previous: Option<&'data [u8]> = None;
        let mut text_keys: Option<bool> = None;
        for _ in 0..count {
            let start = self.position;
            let key = self.item(inner, false)?;
            let encoded_key = self
                .data
                .get(start..self.position)
                .ok_or(self.error(error_code_t::truncated))?;
            let is_text = match key {
                value_t::unsigned(_) | value_t::negative(_) => false,
                value_t::text(_) => true,
                _ => return self.fail(error_code_t::map_key_type),
            };
            if text_keys.is_some_and(|expected| expected != is_text) {
                return self.fail(error_code_t::map_key_type);
            }
            text_keys = Some(is_text);
            if previous.is_some_and(|earlier| encoded_key <= earlier) {
                return self.fail(error_code_t::unsorted_or_duplicate_key);
            }
            previous = Some(encoded_key);
            let value = self.item(inner, false)?;
            pairs.push((key, value));
        }
        Ok(value_t::map(pairs))
    }
}

/// Deterministic encoding (RFC 8949 §4.2.1 core deterministic): shortest arguments,
/// definite lengths and map keys in ascending order of their encodings.
pub fn encode(value: &value_t) -> Vec<u8> {
    let mut output = Vec::new();
    encode_into(value, &mut output);
    output
}

fn length_of(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}

fn head(output: &mut Vec<u8>, major: u8, argument: u64) {
    if let Ok(small) = u8::try_from(argument) {
        if small < 24 {
            output.push(major | small);
        } else {
            output.push(major | 24);
            output.push(small);
        }
    } else if let Ok(short) = u16::try_from(argument) {
        output.push(major | 25);
        output.extend_from_slice(&short.to_be_bytes());
    } else if let Ok(word) = u32::try_from(argument) {
        output.push(major | 26);
        output.extend_from_slice(&word.to_be_bytes());
    } else {
        output.push(major | 27);
        output.extend_from_slice(&argument.to_be_bytes());
    }
}

fn encode_into(value: &value_t, output: &mut Vec<u8>) {
    match value {
        value_t::unsigned(number) => head(output, MajorUnsigned, *number),
        value_t::negative(number) => head(output, MajorNegative, *number),
        value_t::bytes(bytes) => {
            head(output, MajorBytes, length_of(bytes.len()));
            output.extend_from_slice(bytes);
        }
        value_t::text(text) => {
            head(output, MajorText, length_of(text.len()));
            output.extend_from_slice(text.as_bytes());
        }
        value_t::array(items) => {
            head(output, MajorArray, length_of(items.len()));
            for item in items {
                encode_into(item, output);
            }
        }
        value_t::map(pairs) => {
            let mut encoded: Vec<(Vec<u8>, Vec<u8>)> = pairs
                .iter()
                .map(|(key, item)| (encode(key), encode(item)))
                .collect();
            encoded.sort();
            head(output, MajorMap, length_of(encoded.len()));
            for (key, item) in encoded {
                output.extend_from_slice(&key);
                output.extend_from_slice(&item);
            }
        }
        value_t::boolean(false) => output.push(EncodedFalse),
        value_t::boolean(true) => output.push(EncodedTrue),
        value_t::tag(number, content) => {
            head(output, MajorTag, *number);
            encode_into(content, output);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_error(data: &[u8]) -> Option<&'static str> {
        decode(data, layer_t::payload_decode, false)
            .err()
            .map(|error| error.code.as_str())
    }

    #[test]
    fn shortest_heads_round_trip() {
        for number in [
            0u64,
            23,
            24,
            255,
            256,
            65_535,
            65_536,
            0xffff_ffff,
            0x1_0000_0000,
            u64::MAX,
        ] {
            let encoded = encode(&value_t::unsigned(number));
            assert_eq!(
                decode(&encoded, layer_t::payload_decode, false),
                Ok(value_t::unsigned(number))
            );
        }
    }

    #[test]
    fn non_minimal_heads_are_rejected() {
        assert_eq!(decode_error(&[0x18, 0x17]), Some("non_minimal_encoding"));
        assert_eq!(
            decode_error(&[0x19, 0x00, 0xff]),
            Some("non_minimal_encoding")
        );
        assert_eq!(
            decode_error(&[0x1a, 0x00, 0x00, 0xff, 0xff]),
            Some("non_minimal_encoding")
        );
        assert_eq!(
            decode_error(&[0x1b, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]),
            Some("non_minimal_encoding")
        );
        assert_eq!(
            decode_error(&[0x58, 0x01, 0x00]),
            Some("non_minimal_encoding")
        );
    }

    #[test]
    fn forbidden_items_are_rejected() {
        assert_eq!(decode_error(&[0xf6]), Some("simple_value_forbidden"));
        assert_eq!(decode_error(&[0xf8, 0x20]), Some("simple_value_forbidden"));
        assert_eq!(decode_error(&[0xf9, 0x00, 0x00]), Some("float_forbidden"));
        assert_eq!(decode_error(&[0xf9, 0x00]), Some("truncated"));
        assert_eq!(decode_error(&[0x5f]), Some("indefinite_length"));
        assert_eq!(decode_error(&[0x1c]), Some("reserved_additional_info"));
        assert_eq!(decode_error(&[0xd2, 0x80]), Some("tag_forbidden"));
        assert_eq!(
            decode_error(&[0x3b, 0x80, 0, 0, 0, 0, 0, 0, 0]),
            Some("integer_out_of_range")
        );
        assert_eq!(decode_error(&[0x62, 0xc3, 0x28]), Some("invalid_utf8"));
        assert_eq!(decode_error(&[0x00, 0x00]), Some("trailing_bytes"));
        assert_eq!(decode_error(&[]), Some("truncated"));
    }

    #[test]
    fn signed_object_tag_only_at_root() {
        assert!(decode(&[0xd2, 0x80], layer_t::envelope_decode, true).is_ok());
        assert_eq!(
            decode(&[0x81, 0xd2, 0x80], layer_t::envelope_decode, true)
                .err()
                .map(|error| error.code.as_str()),
            Some("tag_forbidden")
        );
        assert_eq!(
            decode(&[0xd3, 0x80], layer_t::envelope_decode, true)
                .err()
                .map(|error| error.code.as_str()),
            Some("tag_forbidden")
        );
    }

    #[test]
    fn map_keys_must_ascend_and_share_a_type() {
        assert_eq!(
            decode_error(&[0xa2, 0x02, 0x00, 0x01, 0x00]),
            Some("unsorted_or_duplicate_key")
        );
        assert_eq!(
            decode_error(&[0xa2, 0x01, 0x00, 0x01, 0x00]),
            Some("unsorted_or_duplicate_key")
        );
        assert_eq!(
            decode_error(&[0xa2, 0x01, 0x00, 0x61, 0x61, 0x00]),
            Some("map_key_type")
        );
        assert_eq!(decode_error(&[0xa1, 0x40, 0x00]), Some("map_key_type"));
        // Encoded-key order: a shorter text key sorts first.
        assert!(
            decode(
                &[0xa2, 0x61, 0x7a, 0x00, 0x62, 0x61, 0x61, 0x00],
                layer_t::payload_decode,
                false
            )
            .is_ok()
        );
        assert_eq!(
            decode_error(&[0xa2, 0x62, 0x61, 0x61, 0x00, 0x61, 0x7a, 0x00]),
            Some("unsorted_or_duplicate_key")
        );
    }

    #[test]
    fn limits_are_checked_before_allocation() {
        let mut deep = vec![0x81u8; MaxDepth];
        deep.push(0x00);
        assert!(decode(&deep, layer_t::payload_decode, false).is_ok());
        let mut too_deep = vec![0x81u8; MaxDepth + 1];
        too_deep.push(0x00);
        assert_eq!(decode_error(&too_deep), Some("depth_exceeded"));
        assert_eq!(
            decode_error(&[0x99, 0x01, 0x01]),
            Some("container_too_large")
        );
        assert_eq!(decode_error(&[0x79, 0x04, 0x01]), Some("text_too_long"));
        assert_eq!(
            decode_error(&[0x5a, 0x00, 0x01, 0x00, 0x00]),
            Some("input_too_large")
        );
        assert_eq!(decode_error(&[0x59, 0xf0, 0x01]), Some("input_too_large"));
        assert_eq!(
            decode_error(&[0x5b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]),
            Some("input_too_large")
        );
        assert_eq!(decode_error(&[0x59, 0xf0, 0x00]), Some("truncated"));
        assert_eq!(
            decode_error(&vec![0u8; MaxEncodedBytes + 1]),
            Some("input_too_large")
        );
    }

    #[test]
    fn encoder_sorts_map_keys_by_encoding() {
        let value = value_t::text_map(vec![
            ("bb", value_t::unsigned(1)),
            ("a", value_t::unsigned(2)),
            ("c", value_t::unsigned(3)),
        ]);
        assert_eq!(
            encode(&value),
            vec![
                0xa3, 0x61, 0x61, 0x02, 0x61, 0x63, 0x03, 0x62, 0x62, 0x62, 0x01
            ]
        );
        assert_eq!(
            decode(&encode(&value), layer_t::payload_decode, false).map(|decoded| encode(&decoded)),
            Ok(encode(&value))
        );
    }
}

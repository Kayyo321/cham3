//! Verification layers and stable error codes (wire format §§1, 7).

use std::fmt;

/// Verification layer of wire format §1 that reported an error. Layers 1–5 are
/// implemented by this crate; the names match the published vectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum layer_t {
    /// Layer 1: decode of the envelope or of the protected-header bytes.
    envelope_decode,
    /// Layer 2: COSE_Sign1 structure and protected header.
    envelope,
    /// Layer 3: key resolution and strict Ed25519 verification.
    signature,
    /// Layer 4 (decode part): decode of the authenticated payload bytes.
    payload_decode,
    /// Layer 4 (schema part): closed schema and cross-field identity rules.
    schema,
    /// Layer 5: stateless commit semantics.
    intrinsic,
}

impl layer_t {
    /// The layer name used by the published vectors.
    pub fn as_str(self) -> &'static str {
        match self {
            layer_t::envelope_decode => "envelope_decode",
            layer_t::envelope => "envelope",
            layer_t::signature => "signature",
            layer_t::payload_decode => "payload_decode",
            layer_t::schema => "schema",
            layer_t::intrinsic => "intrinsic",
        }
    }
}

/// Stable error codes of wire format §7 for layers 1–5.
///
/// `list_length` is the code the published vectors and the independent oracle use
/// for a list whose length is outside its CDDL bounds, too short or too long
/// (wire format §6.1 and §7 name it `list_too_long`; see the M2-003 handoff).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum error_code_t {
    // Decode.
    input_too_large,
    truncated,
    trailing_bytes,
    reserved_additional_info,
    indefinite_length,
    non_minimal_encoding,
    integer_out_of_range,
    float_forbidden,
    simple_value_forbidden,
    tag_forbidden,
    invalid_utf8,
    text_too_long,
    depth_exceeded,
    container_too_large,
    unsorted_or_duplicate_key,
    map_key_type,
    // Envelope.
    not_signed_object,
    protected_header_invalid,
    unsupported_algorithm,
    wrong_object_type,
    unprotected_not_empty,
    signature_length,
    // Schema.
    unknown_field,
    missing_field,
    wrong_type,
    value_out_of_range,
    invalid_text,
    invalid_time,
    invalid_enum,
    list_length,
    list_not_strictly_ascending,
    subject_actor_mismatch,
    lineage_mismatch,
    key_id_mismatch,
    // Signature.
    unknown_key,
    signature_invalid,
    // Intrinsic semantics.
    commit_type_rule,
    lifetime_rule,
    delegation_rule,
    network_grant_rule,
    identity_transition_rule,
    delta_rule,
    result_state_mismatch,
    base_state_mismatch,
}

impl error_code_t {
    /// The stable code string of wire format §7.
    pub fn as_str(self) -> &'static str {
        match self {
            error_code_t::input_too_large => "input_too_large",
            error_code_t::truncated => "truncated",
            error_code_t::trailing_bytes => "trailing_bytes",
            error_code_t::reserved_additional_info => "reserved_additional_info",
            error_code_t::indefinite_length => "indefinite_length",
            error_code_t::non_minimal_encoding => "non_minimal_encoding",
            error_code_t::integer_out_of_range => "integer_out_of_range",
            error_code_t::float_forbidden => "float_forbidden",
            error_code_t::simple_value_forbidden => "simple_value_forbidden",
            error_code_t::tag_forbidden => "tag_forbidden",
            error_code_t::invalid_utf8 => "invalid_utf8",
            error_code_t::text_too_long => "text_too_long",
            error_code_t::depth_exceeded => "depth_exceeded",
            error_code_t::container_too_large => "container_too_large",
            error_code_t::unsorted_or_duplicate_key => "unsorted_or_duplicate_key",
            error_code_t::map_key_type => "map_key_type",
            error_code_t::not_signed_object => "not_signed_object",
            error_code_t::protected_header_invalid => "protected_header_invalid",
            error_code_t::unsupported_algorithm => "unsupported_algorithm",
            error_code_t::wrong_object_type => "wrong_object_type",
            error_code_t::unprotected_not_empty => "unprotected_not_empty",
            error_code_t::signature_length => "signature_length",
            error_code_t::unknown_field => "unknown_field",
            error_code_t::missing_field => "missing_field",
            error_code_t::wrong_type => "wrong_type",
            error_code_t::value_out_of_range => "value_out_of_range",
            error_code_t::invalid_text => "invalid_text",
            error_code_t::invalid_time => "invalid_time",
            error_code_t::invalid_enum => "invalid_enum",
            error_code_t::list_length => "list_length",
            error_code_t::list_not_strictly_ascending => "list_not_strictly_ascending",
            error_code_t::subject_actor_mismatch => "subject_actor_mismatch",
            error_code_t::lineage_mismatch => "lineage_mismatch",
            error_code_t::key_id_mismatch => "key_id_mismatch",
            error_code_t::unknown_key => "unknown_key",
            error_code_t::signature_invalid => "signature_invalid",
            error_code_t::commit_type_rule => "commit_type_rule",
            error_code_t::lifetime_rule => "lifetime_rule",
            error_code_t::delegation_rule => "delegation_rule",
            error_code_t::network_grant_rule => "network_grant_rule",
            error_code_t::identity_transition_rule => "identity_transition_rule",
            error_code_t::delta_rule => "delta_rule",
            error_code_t::result_state_mismatch => "result_state_mismatch",
            error_code_t::base_state_mismatch => "base_state_mismatch",
        }
    }
}

/// The single error reported for a rejected object: the first failure in layer
/// order, and within a decode the first failure in byte order (wire format §7).
/// It carries no input bytes, so it can be logged without exposing content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct format_error_t {
    pub layer: layer_t,
    pub code: error_code_t,
}

impl format_error_t {
    pub fn new(layer: layer_t, code: error_code_t) -> format_error_t {
        format_error_t { layer, code }
    }
}

impl fmt::Display for format_error_t {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.layer.as_str(), self.code.as_str())
    }
}

impl std::error::Error for format_error_t {}

/// Result type of this crate.
pub type format_result_t<ok_t> = Result<ok_t, format_error_t>;

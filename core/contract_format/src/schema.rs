//! Closed payload schemas (docs/specs/m2_ecr_v1.cddl; wire format §§3, 6.1):
//! layer 4. Fields are checked in ascending encoded-key order; within a field, type,
//! then format and limits, then ordering (wire format §7). Every parsed structure
//! keeps every field, so `to_value` reproduces the authenticated bytes exactly.

use crate::dcbor::{encode, value_t};
use crate::error::{error_code_t, format_error_t, format_result_t, layer_t};
use crate::hash::{digest_t, key_id, lineage_id};
use crate::text::{
    is_label_text, is_message_text, is_operation_text, is_package_identity_text, is_resource_text,
    is_valid_time,
};

/// A 16-byte random identifier (`grant_id`, `request_id`, `session_id`, …).
pub type random_id_t = [u8; 16];

/// The only schema version of v1 objects.
pub const SchemaVersion: u64 = 1;
/// Largest Linux UID accepted (4 294 967 295 is reserved).
pub const MaxUid: u64 = 4_294_967_294;
/// The only `contract_type` of v1 grants.
pub const ResourceAccess: &str = "resource_access";

/// Defines a closed text enumeration whose variant names are the wire strings.
macro_rules! TextEnum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            /// Every member.
            pub const All: &'static [$name] = &[$($name::$variant),+];

            /// The wire string.
            pub fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => stringify!($variant)),+
                }
            }

            /// Parses a wire string.
            pub fn parse(text: &str) -> Option<$name> {
                $name::All.iter().copied().find(|member| member.as_str() == text)
            }
        }
    };
}

TextEnum!(
    /// `commit_type` (wire format §6.2).
    commit_type_t { genesis, revision, revocation, temporary_grant, merge, restore, identity_transition }
);
TextEnum!(
    /// `authority_class` of a commit or a key certificate.
    authority_class_t { persistent, temporary }
);
TextEnum!(
    /// `initiating_program`: the trusted program that proposed a commit.
    initiating_program_t { cham3_authui, cham3_inspector, cham3_portal_broker, cham3_dev_cli }
);
TextEnum!(
    /// `lifetime.kind` (wire format §6.3).
    lifetime_kind_t { persistent, timed, session, one_shot }
);
TextEnum!(
    /// `resource.category`.
    resource_category_t { filesystem, device, audio_input, camera, screen_capture, clipboard, portal, ipc, notification, network }
);
TextEnum!(
    /// `reason` of a key invalidation.
    invalidation_reason_t { compromised, rotated, retired }
);

fn schema_error(code: error_code_t) -> format_error_t {
    format_error_t::new(layer_t::schema, code)
}

// ------------------------------------------------------------------ closed maps

struct field_t {
    name: &'static str,
    required: bool,
}

const fn required(name: &'static str) -> field_t {
    field_t {
        name,
        required: true,
    }
}

const fn optional(name: &'static str) -> field_t {
    field_t {
        name,
        required: false,
    }
}

/// Walks a closed map: the union of schema fields and present keys in ascending
/// encoded-key order; an unknown key is `unknown_field`, an absent required field
/// `missing_field`, and every present field is passed to `visit` in that order.
fn walk_map<'value>(
    value: &'value value_t,
    fields: &[field_t],
    mut visit: impl FnMut(&'static str, &'value value_t) -> format_result_t<()>,
) -> format_result_t<()> {
    let value_t::map(pairs) = value else {
        return Err(schema_error(error_code_t::wrong_type));
    };
    let mut entries: Vec<(Vec<u8>, Option<&field_t>, Option<&'value value_t>)> = fields
        .iter()
        .map(|field| (encode(&value_t::text_of(field.name)), Some(field), None))
        .collect();
    for (key, item) in pairs {
        let encoded = encode(key);
        match entries.iter_mut().find(|(known, field, present)| {
            *known == encoded && field.is_some() && present.is_none()
        }) {
            Some(entry) => entry.2 = Some(item),
            None => entries.push((encoded, None, Some(item))),
        }
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    for (_, field, item) in entries {
        match (field, item) {
            (None, _) => return Err(schema_error(error_code_t::unknown_field)),
            (Some(field), None) if field.required => {
                return Err(schema_error(error_code_t::missing_field));
            }
            (Some(_), None) => {}
            (Some(field), Some(item)) => visit(field.name, item)?,
        }
    }
    Ok(())
}

fn present<item_t>(slot: Option<item_t>) -> format_result_t<item_t> {
    slot.ok_or(schema_error(error_code_t::missing_field))
}

fn unknown<item_t>() -> format_result_t<item_t> {
    Err(schema_error(error_code_t::unknown_field))
}

// ------------------------------------------------------------------ scalars

fn parse_uint(value: &value_t, maximum: u64) -> format_result_t<u64> {
    match value {
        value_t::unsigned(number) if *number <= maximum => Ok(*number),
        value_t::unsigned(_) | value_t::negative(_) => {
            Err(schema_error(error_code_t::value_out_of_range))
        }
        _ => Err(schema_error(error_code_t::wrong_type)),
    }
}

fn parse_uid(value: &value_t) -> format_result_t<u32> {
    let number = parse_uint(value, MaxUid)?;
    u32::try_from(number).map_err(|_| schema_error(error_code_t::value_out_of_range))
}

fn parse_fixed_uint(value: &value_t, expected: u64) -> format_result_t<u64> {
    match value {
        value_t::unsigned(number) if *number == expected => Ok(*number),
        value_t::unsigned(_) | value_t::negative(_) => {
            Err(schema_error(error_code_t::invalid_enum))
        }
        _ => Err(schema_error(error_code_t::wrong_type)),
    }
}

fn parse_fixed_text(value: &value_t, expected: &str) -> format_result_t<()> {
    match value {
        value_t::text(text) if text == expected => Ok(()),
        value_t::text(_) => Err(schema_error(error_code_t::invalid_enum)),
        _ => Err(schema_error(error_code_t::wrong_type)),
    }
}

fn parse_enum<enum_t>(
    value: &value_t,
    parse: fn(&str) -> Option<enum_t>,
) -> format_result_t<enum_t> {
    match value {
        value_t::text(text) => parse(text).ok_or(schema_error(error_code_t::invalid_enum)),
        _ => Err(schema_error(error_code_t::wrong_type)),
    }
}

fn parse_fixed_bytes<const Length: usize>(value: &value_t) -> format_result_t<[u8; Length]> {
    match value {
        value_t::bytes(bytes) => <[u8; Length]>::try_from(bytes.as_slice())
            .map_err(|_| schema_error(error_code_t::value_out_of_range)),
        _ => Err(schema_error(error_code_t::wrong_type)),
    }
}

fn parse_digest(value: &value_t) -> format_result_t<digest_t> {
    parse_fixed_bytes::<32>(value)
}

fn parse_random_id(value: &value_t) -> format_result_t<random_id_t> {
    parse_fixed_bytes::<16>(value)
}

fn parse_bool(value: &value_t) -> format_result_t<bool> {
    match value {
        value_t::boolean(flag) => Ok(*flag),
        _ => Err(schema_error(error_code_t::wrong_type)),
    }
}

fn parse_time(value: &value_t) -> format_result_t<String> {
    match value {
        value_t::text(text) if is_valid_time(text) => Ok(text.clone()),
        value_t::text(_) => Err(schema_error(error_code_t::invalid_time)),
        _ => Err(schema_error(error_code_t::wrong_type)),
    }
}

/// Text with a byte-length range and a content rule.
fn parse_text(
    value: &value_t,
    minimum: usize,
    maximum: usize,
    rule: fn(&str) -> bool,
) -> format_result_t<String> {
    let value_t::text(text) = value else {
        return Err(schema_error(error_code_t::wrong_type));
    };
    if !(minimum..=maximum).contains(&text.len()) {
        return Err(schema_error(error_code_t::value_out_of_range));
    }
    if !rule(text) {
        return Err(schema_error(error_code_t::invalid_text));
    }
    Ok(text.clone())
}

fn parse_label(value: &value_t) -> format_result_t<String> {
    parse_text(value, 1, 64, is_label_text)
}

fn parse_app_id(value: &value_t) -> format_result_t<String> {
    parse_text(value, 1, 255, is_label_text)
}

fn parse_operation(value: &value_t) -> format_result_t<String> {
    parse_text(value, 1, 32, is_operation_text)
}

fn parse_package_identity(value: &value_t) -> format_result_t<String> {
    parse_text(value, 1, 255, is_package_identity_text)
}

/// A list with length bounds whose items are strictly ascending by `order_key`.
fn parse_list<'value, item_t>(
    value: &'value value_t,
    minimum: usize,
    maximum: usize,
    parse: impl Fn(&'value value_t) -> format_result_t<item_t>,
    order_key: impl Fn(&item_t, &value_t) -> Vec<u8>,
) -> format_result_t<Vec<item_t>> {
    let value_t::array(items) = value else {
        return Err(schema_error(error_code_t::wrong_type));
    };
    if !(minimum..=maximum).contains(&items.len()) {
        return Err(schema_error(error_code_t::list_length));
    }
    let parsed = items
        .iter()
        .map(parse)
        .collect::<format_result_t<Vec<item_t>>>()?;
    let keys: Vec<Vec<u8>> = parsed
        .iter()
        .zip(items)
        .map(|(item, raw)| order_key(item, raw))
        .collect();
    if keys
        .windows(2)
        .any(|pair| matches!(pair, [left, right] if left >= right))
    {
        return Err(schema_error(error_code_t::list_not_strictly_ascending));
    }
    Ok(parsed)
}

/// Default list order: the deterministic encoding of each item (so text items order
/// shorter first, like map keys).
fn encoded_order<item_t>(_: &item_t, raw: &value_t) -> Vec<u8> {
    encode(raw)
}

fn label_list(value: &value_t, maximum: usize) -> format_result_t<Vec<String>> {
    parse_list(value, 0, maximum, parse_label, encoded_order)
}

fn operation_list(value: &value_t, minimum: usize) -> format_result_t<Vec<String>> {
    parse_list(value, minimum, 32, parse_operation, encoded_order)
}

fn text_list(items: &[String]) -> value_t {
    value_t::array(items.iter().map(|item| value_t::text_of(item)).collect())
}

fn optional_pair<'item, item_t>(
    pairs: &mut Vec<(&'static str, value_t)>,
    name: &'static str,
    item: &'item Option<item_t>,
    convert: impl Fn(&'item item_t) -> value_t,
) {
    if let Some(item) = item {
        pairs.push((name, convert(item)));
    }
}

// ------------------------------------------------------------------ grants and state

/// `lifetime` of a grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct lifetime_t {
    pub kind: lifetime_kind_t,
    pub effective_start: String,
    pub expiration: Option<String>,
    pub session_id: Option<random_id_t>,
    /// Present (always 1) exactly on one-shot grants (checked by layer 5).
    pub usage_limit: Option<u64>,
}

const LifetimeFields: [field_t; 5] = [
    required("kind"),
    required("effective_start"),
    optional("expiration"),
    optional("session_id"),
    optional("usage_limit"),
];

impl lifetime_t {
    fn parse(value: &value_t) -> format_result_t<lifetime_t> {
        let (mut kind, mut effective_start, mut expiration, mut session_id, mut usage_limit) =
            (None, None, None, None, None);
        walk_map(value, &LifetimeFields, |name, item| {
            match name {
                "kind" => kind = Some(parse_enum(item, lifetime_kind_t::parse)?),
                "effective_start" => effective_start = Some(parse_time(item)?),
                "expiration" => expiration = Some(parse_time(item)?),
                "session_id" => session_id = Some(parse_random_id(item)?),
                "usage_limit" => usage_limit = Some(parse_fixed_uint(item, 1)?),
                _ => return unknown(),
            }
            Ok(())
        })?;
        Ok(lifetime_t {
            kind: present(kind)?,
            effective_start: present(effective_start)?,
            expiration,
            session_id,
            usage_limit,
        })
    }

    pub fn to_value(&self) -> value_t {
        let mut pairs = vec![
            ("kind", value_t::text_of(self.kind.as_str())),
            ("effective_start", value_t::text_of(&self.effective_start)),
        ];
        optional_pair(&mut pairs, "expiration", &self.expiration, |item| {
            value_t::text_of(item)
        });
        optional_pair(&mut pairs, "session_id", &self.session_id, |item| {
            value_t::bytes_of(item)
        });
        optional_pair(&mut pairs, "usage_limit", &self.usage_limit, |item| {
            value_t::unsigned(*item)
        });
        value_t::text_map(pairs)
    }
}

/// `resource` of a grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct resource_t {
    pub category: resource_category_t,
    pub resource_id: String,
    pub resource_owner_uid: u32,
    pub operations: Vec<String>,
}

const ResourceFields: [field_t; 4] = [
    required("category"),
    required("resource_id"),
    required("resource_owner_uid"),
    required("operations"),
];

impl resource_t {
    fn parse(value: &value_t) -> format_result_t<resource_t> {
        let (mut category, mut resource_id, mut resource_owner_uid, mut operations) =
            (None, None, None, None);
        walk_map(value, &ResourceFields, |name, item| {
            match name {
                "category" => category = Some(parse_enum(item, resource_category_t::parse)?),
                "resource_id" => resource_id = Some(parse_text(item, 1, 512, is_resource_text)?),
                "resource_owner_uid" => resource_owner_uid = Some(parse_uid(item)?),
                "operations" => operations = Some(operation_list(item, 1)?),
                _ => return unknown(),
            }
            Ok(())
        })?;
        Ok(resource_t {
            category: present(category)?,
            resource_id: present(resource_id)?,
            resource_owner_uid: present(resource_owner_uid)?,
            operations: present(operations)?,
        })
    }

    pub fn to_value(&self) -> value_t {
        value_t::text_map(vec![
            ("category", value_t::text_of(self.category.as_str())),
            ("resource_id", value_t::text_of(&self.resource_id)),
            (
                "resource_owner_uid",
                value_t::unsigned(u64::from(self.resource_owner_uid)),
            ),
            ("operations", text_list(&self.operations)),
        ])
    }
}

/// `delegation` of a grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct delegation_t {
    pub permitted: bool,
    pub max_depth: u64,
    pub max_operations: Vec<String>,
    pub parent_grant_id: Option<random_id_t>,
}

const DelegationFields: [field_t; 4] = [
    required("permitted"),
    required("max_depth"),
    required("max_operations"),
    optional("parent_grant_id"),
];

impl delegation_t {
    fn parse(value: &value_t) -> format_result_t<delegation_t> {
        let (mut permitted, mut max_depth, mut max_operations, mut parent_grant_id) =
            (None, None, None, None);
        walk_map(value, &DelegationFields, |name, item| {
            match name {
                "permitted" => permitted = Some(parse_bool(item)?),
                "max_depth" => max_depth = Some(parse_uint(item, 8)?),
                "max_operations" => max_operations = Some(operation_list(item, 0)?),
                "parent_grant_id" => parent_grant_id = Some(parse_random_id(item)?),
                _ => return unknown(),
            }
            Ok(())
        })?;
        Ok(delegation_t {
            permitted: present(permitted)?,
            max_depth: present(max_depth)?,
            max_operations: present(max_operations)?,
            parent_grant_id,
        })
    }

    pub fn to_value(&self) -> value_t {
        let mut pairs = vec![
            ("permitted", value_t::boolean(self.permitted)),
            ("max_depth", value_t::unsigned(self.max_depth)),
            ("max_operations", text_list(&self.max_operations)),
        ];
        optional_pair(
            &mut pairs,
            "parent_grant_id",
            &self.parent_grant_id,
            |item| value_t::bytes_of(item),
        );
        value_t::text_map(pairs)
    }
}

/// One grant (`contract_type` is always `resource_access` in v1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct grant_t {
    pub grant_id: random_id_t,
    pub lifetime: lifetime_t,
    pub resource: resource_t,
    pub delegation: delegation_t,
}

const GrantFields: [field_t; 5] = [
    required("grant_id"),
    required("contract_type"),
    required("lifetime"),
    required("resource"),
    required("delegation"),
];

impl grant_t {
    fn parse(value: &value_t) -> format_result_t<grant_t> {
        let (mut grant_id, mut contract_type, mut lifetime, mut resource, mut delegation) =
            (None, None, None, None, None);
        walk_map(value, &GrantFields, |name, item| {
            match name {
                "grant_id" => grant_id = Some(parse_random_id(item)?),
                "contract_type" => contract_type = Some(parse_fixed_text(item, ResourceAccess)?),
                "lifetime" => lifetime = Some(lifetime_t::parse(item)?),
                "resource" => resource = Some(resource_t::parse(item)?),
                "delegation" => delegation = Some(delegation_t::parse(item)?),
                _ => return unknown(),
            }
            Ok(())
        })?;
        present(contract_type)?;
        Ok(grant_t {
            grant_id: present(grant_id)?,
            lifetime: present(lifetime)?,
            resource: present(resource)?,
            delegation: present(delegation)?,
        })
    }

    pub fn to_value(&self) -> value_t {
        value_t::text_map(vec![
            ("grant_id", value_t::bytes_of(&self.grant_id)),
            ("contract_type", value_t::text_of(ResourceAccess)),
            ("lifetime", self.lifetime.to_value()),
            ("resource", self.resource.to_value()),
            ("delegation", self.delegation.to_value()),
        ])
    }

    /// Whether this is a network grant (wire format §6.10).
    pub fn is_network(&self) -> bool {
        self.resource.category == resource_category_t::network
    }
}

fn grant_list(value: &value_t, maximum: usize) -> format_result_t<Vec<grant_t>> {
    parse_list(value, 0, maximum, grant_t::parse, |grant: &grant_t, _| {
        grant.grant_id.to_vec()
    })
}

fn grant_list_value(grants: &[grant_t]) -> value_t {
    value_t::array(grants.iter().map(grant_t::to_value).collect())
}

/// `contract_state`: grants strictly ascending by `grant_id`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct contract_state_t {
    pub grants: Vec<grant_t>,
}

const StateFields: [field_t; 1] = [required("grants")];

impl contract_state_t {
    fn parse(value: &value_t) -> format_result_t<contract_state_t> {
        let mut grants = None;
        walk_map(value, &StateFields, |name, item| {
            match name {
                "grants" => grants = Some(grant_list(item, 256)?),
                _ => return unknown(),
            }
            Ok(())
        })?;
        Ok(contract_state_t {
            grants: present(grants)?,
        })
    }

    pub fn to_value(&self) -> value_t {
        value_t::text_map(vec![("grants", grant_list_value(&self.grants))])
    }

    /// `state_hash` (wire format §5).
    pub fn state_hash(&self) -> digest_t {
        crate::hash::state_hash_of_encoding(&encode(&self.to_value()))
    }
}

// ------------------------------------------------------------------ commit payload

/// `actor`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct actor_t {
    pub uid: u32,
    pub user_id: digest_t,
}

/// `authorization`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct authorization_t {
    pub request_id: random_id_t,
    pub session_id: random_id_t,
}

/// `policy` binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct policy_binding_t {
    pub epoch: u64,
    pub version: String,
    pub security_profile: String,
    pub mandatory_constraints: Vec<String>,
}

/// `subject`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct subject_t {
    pub owner_uid: u32,
    pub owner_user_id: digest_t,
    pub app_id: String,
    pub installation_id: random_id_t,
    pub package_identity: String,
    pub executable_sha256: digest_t,
    pub runtime_scope: Option<String>,
}

/// `identity_reference`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct identity_reference_t {
    pub package_identity: String,
    pub executable_sha256: digest_t,
}

/// `identity_change` of an identity transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct identity_change_t {
    pub from: identity_reference_t,
    pub to: identity_reference_t,
}

/// `delta`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct delta_t {
    pub add_grants: Vec<grant_t>,
    pub remove_grant_ids: Vec<random_id_t>,
    pub identity_change: Option<identity_change_t>,
}

/// An ECR commit payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct commit_payload_t {
    pub commit_type: commit_type_t,
    pub timestamp: String,
    pub actor: actor_t,
    pub key_id: digest_t,
    pub authority_class: authority_class_t,
    pub authorization: authorization_t,
    pub policy: policy_binding_t,
    pub subject: subject_t,
    pub initiating_program: initiating_program_t,
    pub lineage_id: digest_t,
    pub parents: Vec<digest_t>,
    pub base_state_hash: digest_t,
    pub merge_base: Option<digest_t>,
    pub restore_reference: Option<digest_t>,
    pub message: String,
    pub delta: delta_t,
    pub result_state: contract_state_t,
}

const ActorFields: [field_t; 2] = [required("uid"), required("user_id")];
const AuthorizationFields: [field_t; 2] = [required("request_id"), required("session_id")];
const PolicyFields: [field_t; 4] = [
    required("epoch"),
    required("version"),
    required("security_profile"),
    required("mandatory_constraints"),
];
const SubjectFields: [field_t; 7] = [
    required("owner_uid"),
    required("owner_user_id"),
    required("app_id"),
    required("installation_id"),
    required("package_identity"),
    required("executable_sha256"),
    optional("runtime_scope"),
];
const IdentityReferenceFields: [field_t; 2] =
    [required("package_identity"), required("executable_sha256")];
const IdentityChangeFields: [field_t; 2] = [required("from"), required("to")];
const DeltaFields: [field_t; 3] = [
    required("add_grants"),
    required("remove_grant_ids"),
    optional("identity_change"),
];
const CommitFields: [field_t; 18] = [
    required("schema_version"),
    required("commit_type"),
    required("timestamp"),
    required("actor"),
    required("key_id"),
    required("authority_class"),
    required("authorization"),
    required("policy"),
    required("subject"),
    required("initiating_program"),
    required("lineage_id"),
    required("parents"),
    required("base_state_hash"),
    optional("merge_base"),
    optional("restore_reference"),
    required("message"),
    required("delta"),
    required("result_state"),
];

impl actor_t {
    fn parse(value: &value_t) -> format_result_t<actor_t> {
        let (mut uid, mut user_id) = (None, None);
        walk_map(value, &ActorFields, |name, item| {
            match name {
                "uid" => uid = Some(parse_uid(item)?),
                "user_id" => user_id = Some(parse_digest(item)?),
                _ => return unknown(),
            }
            Ok(())
        })?;
        Ok(actor_t {
            uid: present(uid)?,
            user_id: present(user_id)?,
        })
    }

    pub fn to_value(&self) -> value_t {
        value_t::text_map(vec![
            ("uid", value_t::unsigned(u64::from(self.uid))),
            ("user_id", value_t::bytes_of(&self.user_id)),
        ])
    }
}

impl authorization_t {
    fn parse(value: &value_t) -> format_result_t<authorization_t> {
        let (mut request_id, mut session_id) = (None, None);
        walk_map(value, &AuthorizationFields, |name, item| {
            match name {
                "request_id" => request_id = Some(parse_random_id(item)?),
                "session_id" => session_id = Some(parse_random_id(item)?),
                _ => return unknown(),
            }
            Ok(())
        })?;
        Ok(authorization_t {
            request_id: present(request_id)?,
            session_id: present(session_id)?,
        })
    }

    pub fn to_value(&self) -> value_t {
        value_t::text_map(vec![
            ("request_id", value_t::bytes_of(&self.request_id)),
            ("session_id", value_t::bytes_of(&self.session_id)),
        ])
    }
}

impl policy_binding_t {
    fn parse(value: &value_t) -> format_result_t<policy_binding_t> {
        let (mut epoch, mut version, mut security_profile, mut mandatory_constraints) =
            (None, None, None, None);
        walk_map(value, &PolicyFields, |name, item| {
            match name {
                "epoch" => epoch = Some(parse_uint(item, u64::MAX)?),
                "version" => version = Some(parse_label(item)?),
                "security_profile" => security_profile = Some(parse_label(item)?),
                "mandatory_constraints" => mandatory_constraints = Some(label_list(item, 32)?),
                _ => return unknown(),
            }
            Ok(())
        })?;
        Ok(policy_binding_t {
            epoch: present(epoch)?,
            version: present(version)?,
            security_profile: present(security_profile)?,
            mandatory_constraints: present(mandatory_constraints)?,
        })
    }

    pub fn to_value(&self) -> value_t {
        value_t::text_map(vec![
            ("epoch", value_t::unsigned(self.epoch)),
            ("version", value_t::text_of(&self.version)),
            ("security_profile", value_t::text_of(&self.security_profile)),
            (
                "mandatory_constraints",
                text_list(&self.mandatory_constraints),
            ),
        ])
    }
}

impl subject_t {
    fn parse(value: &value_t) -> format_result_t<subject_t> {
        let (mut owner_uid, mut owner_user_id, mut app_id, mut installation_id) =
            (None, None, None, None);
        let (mut package_identity, mut executable_sha256, mut runtime_scope) = (None, None, None);
        walk_map(value, &SubjectFields, |name, item| {
            match name {
                "owner_uid" => owner_uid = Some(parse_uid(item)?),
                "owner_user_id" => owner_user_id = Some(parse_digest(item)?),
                "app_id" => app_id = Some(parse_app_id(item)?),
                "installation_id" => installation_id = Some(parse_random_id(item)?),
                "package_identity" => package_identity = Some(parse_package_identity(item)?),
                "executable_sha256" => executable_sha256 = Some(parse_digest(item)?),
                "runtime_scope" => runtime_scope = Some(parse_label(item)?),
                _ => return unknown(),
            }
            Ok(())
        })?;
        Ok(subject_t {
            owner_uid: present(owner_uid)?,
            owner_user_id: present(owner_user_id)?,
            app_id: present(app_id)?,
            installation_id: present(installation_id)?,
            package_identity: present(package_identity)?,
            executable_sha256: present(executable_sha256)?,
            runtime_scope,
        })
    }

    pub fn to_value(&self) -> value_t {
        let mut pairs = vec![
            ("owner_uid", value_t::unsigned(u64::from(self.owner_uid))),
            ("owner_user_id", value_t::bytes_of(&self.owner_user_id)),
            ("app_id", value_t::text_of(&self.app_id)),
            ("installation_id", value_t::bytes_of(&self.installation_id)),
            ("package_identity", value_t::text_of(&self.package_identity)),
            (
                "executable_sha256",
                value_t::bytes_of(&self.executable_sha256),
            ),
        ];
        optional_pair(&mut pairs, "runtime_scope", &self.runtime_scope, |item| {
            value_t::text_of(item)
        });
        value_t::text_map(pairs)
    }

    /// The subject's identity reference (package identity and executable digest).
    pub fn identity(&self) -> identity_reference_t {
        identity_reference_t {
            package_identity: self.package_identity.clone(),
            executable_sha256: self.executable_sha256,
        }
    }
}

impl identity_reference_t {
    fn parse(value: &value_t) -> format_result_t<identity_reference_t> {
        let (mut package_identity, mut executable_sha256) = (None, None);
        walk_map(value, &IdentityReferenceFields, |name, item| {
            match name {
                "package_identity" => package_identity = Some(parse_package_identity(item)?),
                "executable_sha256" => executable_sha256 = Some(parse_digest(item)?),
                _ => return unknown(),
            }
            Ok(())
        })?;
        Ok(identity_reference_t {
            package_identity: present(package_identity)?,
            executable_sha256: present(executable_sha256)?,
        })
    }

    pub fn to_value(&self) -> value_t {
        value_t::text_map(vec![
            ("package_identity", value_t::text_of(&self.package_identity)),
            (
                "executable_sha256",
                value_t::bytes_of(&self.executable_sha256),
            ),
        ])
    }
}

impl identity_change_t {
    fn parse(value: &value_t) -> format_result_t<identity_change_t> {
        let (mut from, mut to) = (None, None);
        walk_map(value, &IdentityChangeFields, |name, item| {
            match name {
                "from" => from = Some(identity_reference_t::parse(item)?),
                "to" => to = Some(identity_reference_t::parse(item)?),
                _ => return unknown(),
            }
            Ok(())
        })?;
        Ok(identity_change_t {
            from: present(from)?,
            to: present(to)?,
        })
    }

    pub fn to_value(&self) -> value_t {
        value_t::text_map(vec![
            ("from", self.from.to_value()),
            ("to", self.to.to_value()),
        ])
    }
}

impl delta_t {
    fn parse(value: &value_t) -> format_result_t<delta_t> {
        let (mut add_grants, mut remove_grant_ids, mut identity_change) = (None, None, None);
        walk_map(value, &DeltaFields, |name, item| {
            match name {
                "add_grants" => add_grants = Some(grant_list(item, 64)?),
                "remove_grant_ids" => {
                    remove_grant_ids =
                        Some(parse_list(item, 0, 64, parse_random_id, encoded_order)?)
                }
                "identity_change" => identity_change = Some(identity_change_t::parse(item)?),
                _ => return unknown(),
            }
            Ok(())
        })?;
        Ok(delta_t {
            add_grants: present(add_grants)?,
            remove_grant_ids: present(remove_grant_ids)?,
            identity_change,
        })
    }

    pub fn to_value(&self) -> value_t {
        let mut pairs = vec![
            ("add_grants", grant_list_value(&self.add_grants)),
            (
                "remove_grant_ids",
                value_t::array(
                    self.remove_grant_ids
                        .iter()
                        .map(|item| value_t::bytes_of(item))
                        .collect(),
                ),
            ),
        ];
        optional_pair(
            &mut pairs,
            "identity_change",
            &self.identity_change,
            identity_change_t::to_value,
        );
        value_t::text_map(pairs)
    }
}

impl commit_payload_t {
    /// Schema layer for a commit signed under `kid`, including the cross-field
    /// identity rules (`key_id_mismatch`, `subject_actor_mismatch`, `lineage_mismatch`).
    pub fn parse(value: &value_t, kid: &digest_t) -> format_result_t<commit_payload_t> {
        let (mut schema_version, mut commit_type, mut timestamp, mut actor, mut key_id_field) =
            (None, None, None, None, None);
        let (
            mut authority_class,
            mut authorization,
            mut policy,
            mut subject,
            mut initiating_program,
        ) = (None, None, None, None, None);
        let (mut lineage, mut parents, mut base_state_hash, mut merge_base, mut restore_reference) =
            (None, None, None, None, None);
        let (mut message, mut delta, mut result_state) = (None, None, None);
        walk_map(value, &CommitFields, |name, item| {
            match name {
                "schema_version" => schema_version = Some(parse_fixed_uint(item, SchemaVersion)?),
                "commit_type" => commit_type = Some(parse_enum(item, commit_type_t::parse)?),
                "timestamp" => timestamp = Some(parse_time(item)?),
                "actor" => actor = Some(actor_t::parse(item)?),
                "key_id" => key_id_field = Some(parse_digest(item)?),
                "authority_class" => {
                    authority_class = Some(parse_enum(item, authority_class_t::parse)?)
                }
                "authorization" => authorization = Some(authorization_t::parse(item)?),
                "policy" => policy = Some(policy_binding_t::parse(item)?),
                "subject" => subject = Some(subject_t::parse(item)?),
                "initiating_program" => {
                    initiating_program = Some(parse_enum(item, initiating_program_t::parse)?)
                }
                "lineage_id" => lineage = Some(parse_digest(item)?),
                "parents" => parents = Some(parse_list(item, 0, 8, parse_digest, encoded_order)?),
                "base_state_hash" => base_state_hash = Some(parse_digest(item)?),
                "merge_base" => merge_base = Some(parse_digest(item)?),
                "restore_reference" => restore_reference = Some(parse_digest(item)?),
                "message" => message = Some(parse_text(item, 0, 1_024, is_message_text)?),
                "delta" => delta = Some(delta_t::parse(item)?),
                "result_state" => result_state = Some(contract_state_t::parse(item)?),
                _ => return unknown(),
            }
            Ok(())
        })?;
        present(schema_version)?;
        let payload = commit_payload_t {
            commit_type: present(commit_type)?,
            timestamp: present(timestamp)?,
            actor: present(actor)?,
            key_id: present(key_id_field)?,
            authority_class: present(authority_class)?,
            authorization: present(authorization)?,
            policy: present(policy)?,
            subject: present(subject)?,
            initiating_program: present(initiating_program)?,
            lineage_id: present(lineage)?,
            parents: present(parents)?,
            base_state_hash: present(base_state_hash)?,
            merge_base,
            restore_reference,
            message: present(message)?,
            delta: present(delta)?,
            result_state: present(result_state)?,
        };
        if payload.key_id != *kid {
            return Err(schema_error(error_code_t::key_id_mismatch));
        }
        let subject = &payload.subject;
        if subject.owner_uid != payload.actor.uid || subject.owner_user_id != payload.actor.user_id
        {
            return Err(schema_error(error_code_t::subject_actor_mismatch));
        }
        if payload.lineage_id
            != lineage_id(
                &subject.owner_user_id,
                &subject.app_id,
                &subject.installation_id,
            )
        {
            return Err(schema_error(error_code_t::lineage_mismatch));
        }
        Ok(payload)
    }

    pub fn to_value(&self) -> value_t {
        let mut pairs = vec![
            ("schema_version", value_t::unsigned(SchemaVersion)),
            ("commit_type", value_t::text_of(self.commit_type.as_str())),
            ("timestamp", value_t::text_of(&self.timestamp)),
            ("actor", self.actor.to_value()),
            ("key_id", value_t::bytes_of(&self.key_id)),
            (
                "authority_class",
                value_t::text_of(self.authority_class.as_str()),
            ),
            ("authorization", self.authorization.to_value()),
            ("policy", self.policy.to_value()),
            ("subject", self.subject.to_value()),
            (
                "initiating_program",
                value_t::text_of(self.initiating_program.as_str()),
            ),
            ("lineage_id", value_t::bytes_of(&self.lineage_id)),
            (
                "parents",
                value_t::array(
                    self.parents
                        .iter()
                        .map(|item| value_t::bytes_of(item))
                        .collect(),
                ),
            ),
            ("base_state_hash", value_t::bytes_of(&self.base_state_hash)),
            ("message", value_t::text_of(&self.message)),
            ("delta", self.delta.to_value()),
            ("result_state", self.result_state.to_value()),
        ];
        optional_pair(&mut pairs, "merge_base", &self.merge_base, |item| {
            value_t::bytes_of(item)
        });
        optional_pair(
            &mut pairs,
            "restore_reference",
            &self.restore_reference,
            |item| value_t::bytes_of(item),
        );
        value_t::text_map(pairs)
    }
}

// ------------------------------------------------------------------ identity objects

/// Identity enrollment payload (self-signed by the identity key).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct identity_enrollment_t {
    pub user_id: digest_t,
    pub public_key: [u8; 32],
    pub owner_uid: u32,
    pub enrolled: String,
    pub identity_epoch: u64,
}

const EnrollmentFields: [field_t; 6] = [
    required("schema_version"),
    required("user_id"),
    required("public_key"),
    required("owner_uid"),
    required("enrolled"),
    required("identity_epoch"),
];

impl identity_enrollment_t {
    /// Schema layer; `kid` must equal `user_id`, which must be the key id of `public_key`.
    pub fn parse(value: &value_t, kid: &digest_t) -> format_result_t<identity_enrollment_t> {
        let (
            mut schema_version,
            mut user_id,
            mut public_key,
            mut owner_uid,
            mut enrolled,
            mut identity_epoch,
        ) = (None, None, None, None, None, None);
        walk_map(value, &EnrollmentFields, |name, item| {
            match name {
                "schema_version" => schema_version = Some(parse_fixed_uint(item, SchemaVersion)?),
                "user_id" => user_id = Some(parse_digest(item)?),
                "public_key" => public_key = Some(parse_digest(item)?),
                "owner_uid" => owner_uid = Some(parse_uid(item)?),
                "enrolled" => enrolled = Some(parse_time(item)?),
                "identity_epoch" => identity_epoch = Some(parse_uint(item, u64::MAX)?),
                _ => return unknown(),
            }
            Ok(())
        })?;
        present(schema_version)?;
        let payload = identity_enrollment_t {
            user_id: present(user_id)?,
            public_key: present(public_key)?,
            owner_uid: present(owner_uid)?,
            enrolled: present(enrolled)?,
            identity_epoch: present(identity_epoch)?,
        };
        if payload.user_id != *kid || payload.user_id != key_id(&payload.public_key) {
            return Err(schema_error(error_code_t::key_id_mismatch));
        }
        Ok(payload)
    }

    pub fn to_value(&self) -> value_t {
        value_t::text_map(vec![
            ("schema_version", value_t::unsigned(SchemaVersion)),
            ("user_id", value_t::bytes_of(&self.user_id)),
            ("public_key", value_t::bytes_of(&self.public_key)),
            ("owner_uid", value_t::unsigned(u64::from(self.owner_uid))),
            ("enrolled", value_t::text_of(&self.enrolled)),
            ("identity_epoch", value_t::unsigned(self.identity_epoch)),
        ])
    }
}

/// Key certificate payload (signed by the identity key).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct key_certificate_t {
    pub certificate_id: random_id_t,
    pub issued: String,
    pub owner_uid: u32,
    pub owner_user_id: digest_t,
    pub subject_key_id: digest_t,
    pub subject_public_key: [u8; 32],
    pub authority_class: authority_class_t,
    pub key_epoch: u64,
    pub not_before: String,
    pub not_after: Option<String>,
    pub supersedes_key_id: Option<digest_t>,
}

const CertificateFields: [field_t; 12] = [
    required("schema_version"),
    required("certificate_id"),
    required("issued"),
    required("owner_uid"),
    required("owner_user_id"),
    required("subject_key_id"),
    required("subject_public_key"),
    required("authority_class"),
    required("key_epoch"),
    required("not_before"),
    optional("not_after"),
    optional("supersedes_key_id"),
];

impl key_certificate_t {
    /// Schema layer; `kid` must equal `owner_user_id`, and `subject_key_id` must be
    /// the key id of `subject_public_key`.
    pub fn parse(value: &value_t, kid: &digest_t) -> format_result_t<key_certificate_t> {
        let (
            mut schema_version,
            mut certificate_id,
            mut issued,
            mut owner_uid,
            mut owner_user_id,
            mut subject_key_id,
        ) = (None, None, None, None, None, None);
        let (
            mut subject_public_key,
            mut authority_class,
            mut key_epoch,
            mut not_before,
            mut not_after,
            mut supersedes_key_id,
        ) = (None, None, None, None, None, None);
        walk_map(value, &CertificateFields, |name, item| {
            match name {
                "schema_version" => schema_version = Some(parse_fixed_uint(item, SchemaVersion)?),
                "certificate_id" => certificate_id = Some(parse_random_id(item)?),
                "issued" => issued = Some(parse_time(item)?),
                "owner_uid" => owner_uid = Some(parse_uid(item)?),
                "owner_user_id" => owner_user_id = Some(parse_digest(item)?),
                "subject_key_id" => subject_key_id = Some(parse_digest(item)?),
                "subject_public_key" => subject_public_key = Some(parse_digest(item)?),
                "authority_class" => {
                    authority_class = Some(parse_enum(item, authority_class_t::parse)?)
                }
                "key_epoch" => key_epoch = Some(parse_uint(item, u64::MAX)?),
                "not_before" => not_before = Some(parse_time(item)?),
                "not_after" => not_after = Some(parse_time(item)?),
                "supersedes_key_id" => supersedes_key_id = Some(parse_digest(item)?),
                _ => return unknown(),
            }
            Ok(())
        })?;
        present(schema_version)?;
        let payload = key_certificate_t {
            certificate_id: present(certificate_id)?,
            issued: present(issued)?,
            owner_uid: present(owner_uid)?,
            owner_user_id: present(owner_user_id)?,
            subject_key_id: present(subject_key_id)?,
            subject_public_key: present(subject_public_key)?,
            authority_class: present(authority_class)?,
            key_epoch: present(key_epoch)?,
            not_before: present(not_before)?,
            not_after,
            supersedes_key_id,
        };
        if payload.owner_user_id != *kid
            || payload.subject_key_id != key_id(&payload.subject_public_key)
        {
            return Err(schema_error(error_code_t::key_id_mismatch));
        }
        Ok(payload)
    }

    pub fn to_value(&self) -> value_t {
        let mut pairs = vec![
            ("schema_version", value_t::unsigned(SchemaVersion)),
            ("certificate_id", value_t::bytes_of(&self.certificate_id)),
            ("issued", value_t::text_of(&self.issued)),
            ("owner_uid", value_t::unsigned(u64::from(self.owner_uid))),
            ("owner_user_id", value_t::bytes_of(&self.owner_user_id)),
            ("subject_key_id", value_t::bytes_of(&self.subject_key_id)),
            (
                "subject_public_key",
                value_t::bytes_of(&self.subject_public_key),
            ),
            (
                "authority_class",
                value_t::text_of(self.authority_class.as_str()),
            ),
            ("key_epoch", value_t::unsigned(self.key_epoch)),
            ("not_before", value_t::text_of(&self.not_before)),
        ];
        optional_pair(&mut pairs, "not_after", &self.not_after, |item| {
            value_t::text_of(item)
        });
        optional_pair(
            &mut pairs,
            "supersedes_key_id",
            &self.supersedes_key_id,
            |item| value_t::bytes_of(item),
        );
        value_t::text_map(pairs)
    }
}

/// Key invalidation payload (signed by the identity key).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct key_invalidation_t {
    pub invalidation_id: random_id_t,
    pub issued: String,
    pub owner_user_id: digest_t,
    pub target_key_id: digest_t,
    pub reason: invalidation_reason_t,
}

const InvalidationFields: [field_t; 6] = [
    required("schema_version"),
    required("invalidation_id"),
    required("issued"),
    required("owner_user_id"),
    required("target_key_id"),
    required("reason"),
];

impl key_invalidation_t {
    /// Schema layer; `kid` must equal `owner_user_id`.
    pub fn parse(value: &value_t, kid: &digest_t) -> format_result_t<key_invalidation_t> {
        let (
            mut schema_version,
            mut invalidation_id,
            mut issued,
            mut owner_user_id,
            mut target_key_id,
            mut reason,
        ) = (None, None, None, None, None, None);
        walk_map(value, &InvalidationFields, |name, item| {
            match name {
                "schema_version" => schema_version = Some(parse_fixed_uint(item, SchemaVersion)?),
                "invalidation_id" => invalidation_id = Some(parse_random_id(item)?),
                "issued" => issued = Some(parse_time(item)?),
                "owner_user_id" => owner_user_id = Some(parse_digest(item)?),
                "target_key_id" => target_key_id = Some(parse_digest(item)?),
                "reason" => reason = Some(parse_enum(item, invalidation_reason_t::parse)?),
                _ => return unknown(),
            }
            Ok(())
        })?;
        present(schema_version)?;
        let payload = key_invalidation_t {
            invalidation_id: present(invalidation_id)?,
            issued: present(issued)?,
            owner_user_id: present(owner_user_id)?,
            target_key_id: present(target_key_id)?,
            reason: present(reason)?,
        };
        if payload.owner_user_id != *kid {
            return Err(schema_error(error_code_t::key_id_mismatch));
        }
        Ok(payload)
    }

    pub fn to_value(&self) -> value_t {
        value_t::text_map(vec![
            ("schema_version", value_t::unsigned(SchemaVersion)),
            ("invalidation_id", value_t::bytes_of(&self.invalidation_id)),
            ("issued", value_t::text_of(&self.issued)),
            ("owner_user_id", value_t::bytes_of(&self.owner_user_id)),
            ("target_key_id", value_t::bytes_of(&self.target_key_id)),
            ("reason", value_t::text_of(self.reason.as_str())),
        ])
    }
}

//! Scalar text formats (wire format §3).

/// `utc_time`: exactly `YYYY-MM-DDTHH:MM:SSZ` in ASCII, year 2000–9999, a valid
/// Gregorian date, hour 00–23, minute and second 00–59 (no leap second).
pub fn is_valid_time(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 20 {
        return false;
    }
    let separators = [
        (4, b'-'),
        (7, b'-'),
        (10, b'T'),
        (13, b':'),
        (16, b':'),
        (19, b'Z'),
    ];
    if separators
        .iter()
        .any(|(index, expected)| bytes.get(*index) != Some(expected))
    {
        return false;
    }
    let field = |start: usize, end: usize| -> Option<u32> {
        bytes
            .get(start..end)?
            .iter()
            .try_fold(0u32, |number, byte| {
                let digit = char::from(*byte).to_digit(10)?;
                number.checked_mul(10)?.checked_add(digit)
            })
    };
    let parsed = (
        field(0, 4),
        field(5, 7),
        field(8, 10),
        field(11, 13),
        field(14, 16),
        field(17, 19),
    );
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = parsed
    else {
        return false;
    };
    year >= 2000
        && (1..=days_in_month(year, month)).contains(&day)
        && hour <= 23
        && minute <= 59
        && second <= 59
}

fn days_in_month(year: u32, month: u32) -> u32 {
    let divisible = |divisor: u32| year.checked_rem(divisor) == Some(0);
    let leap = divisible(4) && (!divisible(100) || divisible(400));
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    }
}

/// `app_id` and `ascii_label` pattern `[a-z0-9][a-z0-9._-]*` (lengths are checked
/// separately).
pub fn is_label_text(text: &str) -> bool {
    let mut bytes = text.bytes();
    match bytes.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
        _ => return false,
    }
    bytes.all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
    })
}

/// `operation` pattern `[a-z][a-z0-9_]*`.
pub fn is_operation_text(text: &str) -> bool {
    let mut bytes = text.bytes();
    match bytes.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// `package_identity`: printable ASCII 0x21–0x7e.
pub fn is_package_identity_text(text: &str) -> bool {
    text.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

/// Code points rejected in restricted resource text and display text (wire format
/// §3.3): C0 and C1 controls, DEL, the bidirectional controls and U+FEFF.
pub fn is_forbidden_code_point(character: char) -> bool {
    matches!(
        u32::from(character),
        0x00..=0x1f | 0x7f..=0x9f | 0x061c | 0x200e | 0x200f | 0x202a..=0x202e | 0x2066..=0x2069 | 0xfeff
    )
}

/// Restricted resource text content rule (wire format §3.3); lengths are checked
/// separately.
pub fn is_resource_text(text: &str) -> bool {
    !text.chars().any(is_forbidden_code_point)
}

/// Maximum line feeds in a `message` (at most 16 lines).
pub const MaxMessageLineFeeds: usize = 15;

/// Display text content rule (wire format §3.5): LF is the only permitted control,
/// no leading or trailing whitespace on the whole value, at most 16 lines. Lengths
/// are checked separately.
pub fn is_message_text(text: &str) -> bool {
    if text
        .chars()
        .any(|character| character != '\n' && is_forbidden_code_point(character))
    {
        return false;
    }
    if text.trim() != text {
        return false;
    }
    text.bytes().filter(|byte| *byte == b'\n').count() <= MaxMessageLineFeeds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_format_is_exact() {
        assert!(is_valid_time("2026-10-12T12:00:00Z"));
        assert!(is_valid_time("2000-02-29T23:59:59Z"));
        assert!(!is_valid_time("2100-02-29T00:00:00Z"));
        assert!(!is_valid_time("1999-12-31T23:59:59Z"));
        assert!(!is_valid_time("2026-10-12T24:00:00Z"));
        assert!(!is_valid_time("2026-10-12T12:00:60Z"));
        assert!(!is_valid_time("2026-13-12T12:00:00Z"));
        assert!(!is_valid_time("2026-04-31T12:00:00Z"));
        assert!(!is_valid_time("2026-10-12t12:00:00Z"));
        assert!(!is_valid_time("2026-10-12T12:00:00z"));
        assert!(!is_valid_time("2026-10-12T12:00:00Z\n"));
        assert!(!is_valid_time("+026-10-12T12:00:00Z"));
        // Non-ASCII digits are not digits here (20 characters, more than 20 bytes).
        assert!(!is_valid_time(
            "\u{0662}\u{0660}\u{0662}\u{0666}-10-12T12:00:00Z"
        ));
    }

    #[test]
    fn label_and_operation_patterns() {
        assert!(is_label_text("org.example.notes"));
        assert!(is_label_text("0-a_b"));
        assert!(!is_label_text(".a"));
        assert!(!is_label_text("Ab"));
        assert!(!is_label_text("abc\n"));
        assert!(!is_label_text(""));
        assert!(is_operation_text("read_write"));
        assert!(!is_operation_text("1read"));
        assert!(!is_operation_text("read-write"));
    }

    #[test]
    fn restricted_text_rules() {
        assert!(is_resource_text("home:Documents/notes"));
        assert!(!is_resource_text("a\u{202e}b"));
        assert!(!is_resource_text("a\u{0085}b"));
        assert!(is_message_text("first\nsecond"));
        assert!(!is_message_text(" leading"));
        assert!(!is_message_text("trailing\u{00a0}"));
        assert!(!is_message_text("tab\there"));
        assert!(is_message_text(""));
        assert!(!is_message_text(&["x"; 17].join("\n")));
        assert!(is_message_text(&["x"; 16].join("\n")));
    }
}

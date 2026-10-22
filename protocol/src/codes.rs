//! Friend codes (8 characters) and group invite codes (10), in Crockford base32:
//! digits and capitals without I, L, O and U. Shown as "7KQ2-9XMB".

pub const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
pub const FRIEND_CODE_LEN: usize = 8;
pub const GROUP_CODE_LEN: usize = 10;

/// What a person typed or pasted → the stored form. Ignores case, dashes and spaces, and
/// reads the look-alikes I/L as 1 and O as 0. None if it can't be a code of that length.
pub fn normalize(input: &str, len: usize) -> Option<String> {
    let mut out = String::with_capacity(len);
    for c in input.chars() {
        let c = match c.to_ascii_uppercase() {
            '-' | ' ' => continue,
            'I' | 'L' => '1',
            'O' => '0',
            c => c,
        };
        if !c.is_ascii() || !ALPHABET.contains(&(c as u8)) {
            return None;
        }
        out.push(c);
    }
    (out.len() == len).then_some(out)
}

/// One code from random bytes (one byte per character; only the low 5 bits are used).
pub fn from_random(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| ALPHABET[(b & 31) as usize] as char)
        .collect()
}

/// "7KQ29XMB" → "7KQ2-9XMB".
pub fn display(code: &str) -> String {
    let mid = code.len() / 2;
    format!("{}-{}", &code[..mid], &code[mid..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_what_people_type() {
        assert_eq!(
            normalize("7kq2-9xmb", FRIEND_CODE_LEN).as_deref(),
            Some("7KQ29XMB")
        );
        assert_eq!(
            normalize(" 7KQ2 9XMO ", FRIEND_CODE_LEN).as_deref(),
            Some("7KQ29XM0")
        );
        assert_eq!(
            normalize("IL", 2).as_deref(),
            Some("11"),
            "look-alikes read as digits"
        );
        assert_eq!(normalize("7KQ29XM", FRIEND_CODE_LEN), None, "too short");
        assert_eq!(normalize("7KQ29XMU", FRIEND_CODE_LEN), None, "U isn't used");
        assert_eq!(normalize("7KQ2/9XMB", FRIEND_CODE_LEN), None);
    }

    #[test]
    fn random_codes_use_the_alphabet() {
        let code = from_random(&[0, 31, 32, 255, 7, 8, 9, 10]);
        assert_eq!(code, "0Z0Z789A");
        assert_eq!(display(&code), "0Z0Z-789A");
        assert_eq!(normalize(&code, 8).as_deref(), Some(code.as_str()));
    }
}

use std::fmt::Write as _;

pub fn push(text: &mut String, bytes: &[u8]) {
    for byte in bytes {
        write!(text, "{byte:02x}").expect("a string accepts text");
    }
}

pub fn decode<const N: usize>(text: &str) -> Option<[u8; N]> {
    let digits = text.as_bytes();
    if digits.len() != N * 2 {
        return None;
    }
    let mut bytes = [0; N];
    let (pairs, _) = digits.as_chunks::<2>();
    for (byte, [high, low]) in bytes.iter_mut().zip(pairs) {
        *byte = digit(*high)? * 16 + digit(*low)?;
    }
    Some(bytes)
}

fn digit(character: u8) -> Option<u8> {
    match character {
        b'0'..=b'9' => Some(character - b'0'),
        b'a'..=b'f' => Some(character - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{decode, push};

    fn encode(bytes: &[u8]) -> String {
        let mut text = String::new();
        push(&mut text, bytes);
        text
    }

    #[test]
    fn writes_two_lower_case_digits_for_each_byte() {
        assert_eq!(encode(&[]), "");
        assert_eq!(encode(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
    }

    #[test]
    fn adds_to_the_text_that_is_there() {
        let mut text = String::from("x:");
        push(&mut text, &[0x10]);
        assert_eq!(text, "x:10");
    }

    #[test]
    fn reads_the_text_that_it_writes() {
        assert_eq!(decode::<4>("000fa5ff"), Some([0x00, 0x0f, 0xa5, 0xff]));
        assert_eq!(decode::<0>(""), Some([]));
    }

    #[test]
    fn refuses_an_incorrect_length() {
        assert_eq!(decode::<2>("00"), None);
        assert_eq!(decode::<2>("000"), None);
        assert_eq!(decode::<2>("000000"), None);
    }

    #[test]
    fn refuses_a_character_that_is_not_a_lower_case_digit() {
        for text in ["0g", "g0", "0A", "A0", " 0", "0 ", "-1", "\u{e9}"] {
            assert_eq!(decode::<1>(text), None, "{text:?}");
        }
    }
}

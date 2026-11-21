use vstd::prelude::*;

verus! {

pub open spec fn hex_digit(value: int) -> u8 {
    if value < 10 {
        (b'0' + value) as u8
    } else {
        (b'a' + value - 10) as u8
    }
}

pub open spec fn hex_text(bytes: Seq<u8>) -> Seq<u8> {
    Seq::new(
        2 * bytes.len(),
        |i: int| hex_digit(if i % 2 == 0 { bytes[i / 2] as int / 16 } else { bytes[i / 2] as int % 16 }),
    )
}

pub open spec fn hex_characters(bytes: Seq<u8>) -> Seq<char> {
    Seq::new(2 * bytes.len(), |i: int| hex_text(bytes)[i] as char)
}

pub proof fn lemma_a_hex_text_gives_its_bytes(bytes: Seq<u8>, other: Seq<u8>)
    requires
        hex_text(bytes) == hex_text(other),
    ensures
        bytes == other,
{
    assert(bytes.len() == other.len()) by {
        assert(hex_text(bytes).len() == 2 * bytes.len());
        assert(hex_text(other).len() == 2 * other.len());
    }
    assert forall|i: int| 0 <= i < bytes.len() implies bytes[i] == other[i] by {
        let byte = bytes[i] as int;
        let other_byte = other[i] as int;
        assert(hex_text(bytes)[2 * i] == hex_digit(byte / 16));
        assert(hex_text(other)[2 * i] == hex_digit(other_byte / 16));
        assert(hex_text(bytes)[2 * i + 1] == hex_digit(byte % 16));
        assert(hex_text(other)[2 * i + 1] == hex_digit(other_byte % 16));
        assert(byte / 16 == other_byte / 16 && byte % 16 == other_byte % 16);
    }
    assert(bytes =~= other);
}

fn digit(value: u8) -> (digit: u8)
    requires
        value < 16,
    ensures
        digit == hex_digit(value as int),
{
    if value < 10 {
        b'0' + value
    } else {
        b'a' + (value - 10)
    }
}

fn value(digit: u8) -> (value: Option<u8>)
    ensures
        value matches Some(value) ==> value < 16 && digit == hex_digit(value as int),
        value is None ==> forall|value: int| 0 <= value < 16 ==> digit != #[trigger] hex_digit(value),
{
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        _ => None,
    }
}

pub fn push(text: &mut String, bytes: &[u8])
    ensures
        final(text)@ == old(text)@ + hex_characters(bytes@),
{
    let mut index = 0;
    while index < bytes.len()
        invariant
            index <= bytes.len(),
            text@ == old(text)@ + hex_characters(bytes@.take(index as int)),
        decreases bytes.len() - index,
    {
        let byte = bytes[index];
        text.push(digit(byte / 16) as char);
        text.push(digit(byte % 16) as char);
        index += 1;
        assert(text@ =~= old(text)@ + hex_characters(bytes@.take(index as int)));
    }
    assert(bytes@.take(index as int) =~= bytes@);
}

pub fn decode<const N: usize>(digits: &[u8]) -> (bytes: Option<[u8; N]>)
    ensures
        bytes matches Some(bytes) ==> digits@ == hex_text(bytes@),
        bytes is None ==> forall|bytes: Seq<u8>|
            bytes.len() == N ==> digits@ != #[trigger] hex_text(bytes),
{
    if !digits.len().is_multiple_of(2) || digits.len() / 2 != N {
        return None;
    }
    let mut bytes = [0; N];
    let mut index = 0;
    while index < N
        invariant
            digits@.len() == 2 * N <= usize::MAX,
            index <= N,
            forall|i: int|
                0 <= i < index ==> digits@[2 * i] == hex_digit(#[trigger] bytes@[i] as int / 16)
                    && digits@[2 * i + 1] == hex_digit(bytes@[i] as int % 16),
        decreases N - index,
    {
        let high = value(digits[2 * index]);
        let low = value(digits[2 * index + 1]);
        if let (Some(high), Some(low)) = (high, low) {
            bytes[index] = high * 16 + low;
        } else {
            assert forall|other: Seq<u8>| other.len() == N implies digits@ != #[trigger] hex_text(
                other,
            ) by {
                assert(hex_text(other)[2 * index] == hex_digit(other[index as int] as int / 16));
                assert(hex_text(other)[2 * index + 1] == hex_digit(other[index as int] as int % 16));
            }
            return None;
        }
        index += 1;
    }
    assert(digits@ =~= hex_text(bytes@));
    Some(bytes)
}

} // verus!
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
        assert_eq!(decode::<4>(b"000fa5ff"), Some([0x00, 0x0f, 0xa5, 0xff]));
        assert_eq!(decode::<0>(b""), Some([]));
    }

    #[test]
    fn refuses_an_incorrect_length() {
        assert_eq!(decode::<2>(b"00"), None);
        assert_eq!(decode::<2>(b"000"), None);
        assert_eq!(decode::<2>(b"000000"), None);
    }

    #[test]
    fn refuses_a_character_that_is_not_a_lower_case_digit() {
        for text in ["0g", "g0", "0A", "A0", " 0", "0 ", "-1", "\u{e9}"] {
            assert_eq!(decode::<1>(text.as_bytes()), None, "{text:?}");
        }
    }
}

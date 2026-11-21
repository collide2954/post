use post_proto::{Code, Nonce};
#[cfg(verus_keep_ghost)]
use post_proto::{is_code, is_code_character};
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::string::StringSliceAdditionalSpecFns;
#[cfg(verus_keep_ghost)]
use vstd::utf8::{encode_utf8, is_ascii_chars, is_ascii_chars_encode_utf8};

pub(crate) fn bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).expect("the operating system gives random bytes");
    bytes
}

#[must_use]
pub fn nonce() -> Nonce {
    Nonce::from_bytes(bytes())
}

#[must_use]
pub fn code() -> Code {
    code_from(bytes::<32>)
}

impl<F: FnMut() -> [u8; 32]> Source for F {
    fn block(&mut self) -> [u8; 32] {
        self()
    }
}

verus! {

const CHARACTERS: u8 = 62;

const FIRST_UNFAIR_BYTE: u8 = 248;

trait Source {
    fn block(&mut self) -> [u8; 32];
}

pub open spec fn is_fair(byte: u8) -> bool {
    byte < 248
}

pub open spec fn fair_character(byte: u8) -> u8 {
    let index = byte % 62;
    if index < 26 {
        (b'A' + index) as u8
    } else if index < 52 {
        (b'a' + index - 26) as u8
    } else {
        (b'0' + index - 52) as u8
    }
}

pub open spec fn fair_characters(bytes: Seq<u8>) -> Seq<u8>
    decreases bytes.len(),
{
    if bytes.len() == 0 {
        Seq::empty()
    } else if is_fair(bytes.last()) {
        fair_characters(bytes.drop_last()).push(fair_character(bytes.last()))
    } else {
        fair_characters(bytes.drop_last())
    }
}

pub open spec fn first_twelve(characters: Seq<u8>) -> Seq<u8> {
    if characters.len() <= 12 {
        characters
    } else {
        characters.take(12)
    }
}

pub open spec fn has_code_characters(characters: Seq<u8>) -> bool {
    forall|i: int| 0 <= i < characters.len() ==> is_code_character(#[trigger] characters[i])
}

pub proof fn lemma_each_character_comes_from_four_bytes(byte: u8, other: u8)
    requires
        is_fair(byte),
        is_fair(other),
    ensures
        fair_character(byte) == fair_character(other) <==> byte % 62 == other % 62,
        byte % 62 == other % 62 <==> {
            ||| other == byte % 62
            ||| other == byte % 62 + 62
            ||| other == byte % 62 + 124
            ||| other == byte % 62 + 186
        },
{
}

pub proof fn lemma_each_code_character_comes_from_a_byte(character: u8)
    requires
        is_code_character(character),
    ensures
        exists|byte: u8| is_fair(byte) && fair_character(byte) == character,
{
    let byte = if b'A' <= character <= b'Z' {
        (character - b'A') as u8
    } else if b'a' <= character <= b'z' {
        (character - b'a' + 26) as u8
    } else {
        (character - b'0' + 52) as u8
    };
    assert(is_fair(byte) && fair_character(byte) == character);
}

fn character(byte: u8) -> (character: Option<u8>)
    ensures
        character is Some <==> is_fair(byte),
        character matches Some(character) ==> character == fair_character(byte)
            && is_code_character(character),
{
    if byte >= FIRST_UNFAIR_BYTE {
        return None;
    }
    let index = byte % CHARACTERS;
    if index < 26 {
        Some(b'A' + index)
    } else if index < 52 {
        Some(b'a' + (index - 26))
    } else {
        Some(b'0' + (index - 52))
    }
}

fn add_characters(characters: &mut Vec<u8>, block: &[u8; 32])
    requires
        old(characters)@.len() <= 12,
        has_code_characters(old(characters)@),
    ensures
        final(characters)@ == first_twelve(old(characters)@ + fair_characters(block@)),
        final(characters)@.len() <= 12,
        has_code_characters(final(characters)@),
{
    let mut index = 0;
    proof {
        assert(block@.take(0) =~= Seq::<u8>::empty());
        assert(old(characters)@ + fair_characters(block@.take(0)) =~= old(characters)@);
    }
    while index < block.len()
        invariant
            index <= block@.len(),
            characters@.len() <= 12,
            has_code_characters(characters@),
            characters@ == first_twelve(
                old(characters)@ + fair_characters(block@.take(index as int)),
            ),
        decreases block@.len() - index,
    {
        let next = character(block[index]);
        proof {
            let before = block@.take(index as int);
            let after = block@.take(index + 1);
            assert(after.drop_last() =~= before);
            assert(after.last() == block@[index as int]);
        }
        let has_room = characters.len() < Code::LENGTH;
        if let (Some(next), true) = (next, has_room) {
            characters.push(next);
        }
        index += 1;
        assert(characters@ =~= first_twelve(
            old(characters)@ + fair_characters(block@.take(index as int)),
        ));
    }
    assert(block@.take(index as int) =~= block@);
}

fn text_of(characters: &[u8]) -> (text: String)
    requires
        has_code_characters(characters@),
    ensures
        encode_utf8(text@) == characters@,
{
    let mut text = String::new();
    let mut index = 0;
    while index < characters.len()
        invariant
            index <= characters@.len(),
            text@.len() == index,
            forall|i: int| 0 <= i < index ==> #[trigger] text@[i] == characters@[i] as char,
        decreases characters@.len() - index,
    {
        text.push(characters[index] as char);
        index += 1;
    }
    proof {
        assert(is_ascii_chars(text@));
        is_ascii_chars_encode_utf8(text@);
        assert(encode_utf8(text@) =~= characters@);
    }
    text
}

#[verifier::exec_allows_no_decreases_clause]
fn code_from<S: Source>(mut source: S) -> Code {
    let mut characters = Vec::new();
    while characters.len() < Code::LENGTH
        invariant
            characters@.len() <= 12,
            has_code_characters(characters@),
    {
        let block = source.block();
        add_characters(&mut characters, &block);
    }
    let text = text_of(characters.as_slice());
    Code::parse(text.as_str()).expect("the generator gives only letters and digits")
}

} // verus!
#[cfg(test)]
mod tests {
    use super::{bytes, code, code_from, nonce};

    fn counted() -> [u8; 32] {
        std::array::from_fn(|index| u8::try_from(index).expect("a small index"))
    }

    #[test]
    fn gives_different_bytes_each_time() {
        assert_ne!(bytes::<32>(), bytes::<32>());
        assert_ne!(bytes::<32>(), [0; 32]);
    }

    #[test]
    fn gives_a_different_random_value_each_time() {
        assert_ne!(nonce(), nonce());
    }

    #[test]
    fn gives_a_different_code_each_time() {
        assert_ne!(code(), code());
    }

    #[test]
    fn makes_a_code_from_the_first_fair_bytes() {
        assert_eq!(code_from(counted).as_str(), "ABCDEFGHIJKL");
        assert_eq!(code_from(|| [61; 32]).as_str(), "999999999999");
        assert_eq!(code_from(|| [62; 32]).as_str(), "AAAAAAAAAAAA");
        assert_eq!(code_from(|| [247; 32]).as_str(), "999999999999");
    }

    #[test]
    fn ignores_a_byte_that_makes_some_characters_more_frequent() {
        for unfair in [248, 249, 255] {
            let mut calls = 0;
            let code = code_from(|| {
                calls += 1;
                if calls == 1 { [unfair; 32] } else { [1; 32] }
            });

            assert_eq!(code.as_str(), "BBBBBBBBBBBB", "{unfair}");
            assert_eq!(calls, 2, "{unfair}");
        }
    }

    #[test]
    fn asks_for_more_bytes_until_the_code_is_complete() {
        let mut calls = 0;
        let code = code_from(|| {
            calls += 1;
            let mut bytes = [255; 32];
            bytes[0] = calls;
            bytes
        });

        assert_eq!(code.as_str(), "BCDEFGHIJKLM");
        assert_eq!(calls, 12);
    }
}

use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::string::StringSliceAdditionalSpecFns;
#[cfg(verus_keep_ghost)]
use vstd::utf8::{encode_utf8, is_ascii_chars, is_ascii_chars_encode_utf8};

use crate::hex;
#[cfg(verus_keep_ghost)]
use crate::hex::{hex_characters, hex_digit, hex_text};

verus! {

pub const LENGTH: usize = 76;

pub open spec fn label() -> Seq<u8> {
    "post-seed-1:".spec_bytes()
}

pub open spec fn seed_text(seed: Seq<u8>) -> Seq<u8> {
    label() + hex_text(seed)
}

pub open spec fn is_seed_file(text: Seq<u8>, seed: Seq<u8>) -> bool {
    ||| text == seed_text(seed)
    ||| text == seed_text(seed) + seq![b'\n']
    ||| text == seed_text(seed) + seq![b'\r', b'\n']
}

pub open spec fn line_of(text: Seq<u8>) -> Seq<u8> {
    let length = text.len() as int;
    if length >= 2 && text[length - 2] == b'\r' && text[length - 1] == b'\n' {
        text.take(length - 2)
    } else if length >= 1 && text[length - 1] == b'\n' {
        text.take(length - 1)
    } else {
        text
    }
}

proof fn lemma_the_label()
    ensures
        label().len() == 12,
        "post-seed-1:"@.len() == 12,
        forall|i: int| 0 <= i < 12 ==> #[trigger] label()[i] == "post-seed-1:"@[i] as u8,
        is_ascii_chars("post-seed-1:"@),
{
    reveal_strlit("post-seed-1:");
    assert(is_ascii_chars("post-seed-1:"@));
    is_ascii_chars_encode_utf8("post-seed-1:"@);
}

proof fn lemma_a_hex_digit_is_no_line_end(value: int)
    requires
        0 <= value < 16,
    ensures
        hex_digit(value) != b'\r',
        hex_digit(value) != b'\n',
        hex_digit(value) < 128,
{
}

pub proof fn lemma_the_line_of_a_seed_file(text: Seq<u8>, seed: Seq<u8>)
    requires
        seed.len() == 32,
    ensures
        is_seed_file(text, seed) <==> line_of(text) == seed_text(seed),
{
    lemma_the_label();
    let line = seed_text(seed);
    assert(line.len() == 76);
    let last = seed[31] as int % 16;
    assert(line[75] == hex_text(seed)[63]);
    assert(hex_text(seed)[63] == hex_digit(last));
    lemma_a_hex_digit_is_no_line_end(last);
    if text == line {
        assert(line_of(text) == line);
    } else if text == line + seq![b'\n'] {
        assert(text[76] == b'\n' && text[75] == line[75]);
        assert(line_of(text) =~= line);
    } else if text == line + seq![b'\r', b'\n'] {
        assert(text[76] == b'\r' && text[77] == b'\n');
        assert(line_of(text) =~= line);
    }
    if line_of(text) == line {
        let length = text.len() as int;
        if length >= 2 && text[length - 2] == b'\r' && text[length - 1] == b'\n' {
            assert(text =~= line + seq![b'\r', b'\n']);
        } else if length >= 1 && text[length - 1] == b'\n' {
            assert(text =~= line + seq![b'\n']);
        }
    }
}

proof fn lemma_the_bytes_of_a_written_seed(seed: Seq<u8>)
    requires
        seed.len() == 32,
    ensures
        encode_utf8("post-seed-1:"@ + hex_characters(seed)) == seed_text(seed),
{
    lemma_the_label();
    let characters = hex_characters(seed);
    let digits = hex_text(seed);
    let text = "post-seed-1:"@ + characters;
    assert forall|i: int| 0 <= i < 64 implies #[trigger] characters[i] == digits[i] as char
        && digits[i] < 128 by {
        let byte = seed[i / 2] as int;
        lemma_a_hex_digit_is_no_line_end(byte / 16);
        lemma_a_hex_digit_is_no_line_end(byte % 16);
    }
    assert(is_ascii_chars(text));
    is_ascii_chars_encode_utf8(text);
    assert forall|i: int| 0 <= i < 76 implies #[trigger] encode_utf8(text)[i] == seed_text(seed)[i] by {
        assert(encode_utf8(text)[i] == text[i] as u8);
        if i < 12 {
            assert(text[i] == "post-seed-1:"@[i]);
            assert(seed_text(seed)[i] == label()[i]);
        } else {
            assert(text[i] == characters[i - 12]);
            assert(seed_text(seed)[i] == digits[i - 12]);
        }
    }
    assert(encode_utf8(text) =~= seed_text(seed));
}

fn without_line_end(text: &[u8]) -> (line: &[u8])
    ensures
        line@ == line_of(text@),
{
    let length = text.len();
    if length >= 2 && text[length - 2] == b'\r' && text[length - 1] == b'\n' {
        &text[..length - 2]
    } else if length >= 1 && text[length - 1] == b'\n' {
        &text[..length - 1]
    } else {
        text
    }
}

fn starts_with_the_label(line: &[u8]) -> (starts: bool)
    requires
        line@.len() >= 12,
    ensures
        starts == (line@.take(12) == label()),
{
    proof {
        lemma_the_label();
    }
    let start = "post-seed-1:".as_bytes();
    let mut index = 0;
    while index < 12
        invariant
            index <= 12,
            line@.len() >= 12,
            start@ == label(),
            start@.len() == 12,
            forall|i: int| 0 <= i < index ==> line@[i] == #[trigger] start@[i],
        decreases 12 - index,
    {
        if line[index] != start[index] {
            assert(line@.take(12)[index as int] != label()[index as int]);
            return false;
        }
        index += 1;
    }
    assert(line@.take(12) =~= label());
    true
}

pub fn read(text: &str) -> (seed: Option<[u8; 32]>)
    ensures
        seed matches Some(seed) ==> is_seed_file(text.spec_bytes(), seed@),
        seed is None ==> forall|seed: Seq<u8>|
            seed.len() == 32 ==> !#[trigger] is_seed_file(text.spec_bytes(), seed),
{
    let line = without_line_end(text.as_bytes());
    proof {
        lemma_the_label();
        assert forall|seed: Seq<u8>| seed.len() == 32 implies (is_seed_file(text.spec_bytes(), seed)
            <==> line@ == #[trigger] seed_text(seed)) by {
            lemma_the_line_of_a_seed_file(text.spec_bytes(), seed);
        }
    }
    if line.len() != LENGTH || !starts_with_the_label(line) {
        assert forall|seed: Seq<u8>| seed.len() == 32 implies line@ != #[trigger] seed_text(seed) by {
            assert(seed_text(seed).len() == 76);
            assert(seed_text(seed).take(12) =~= label());
        }
        return None;
    }
    let digits = &line[12..];
    let seed = hex::decode::<32>(digits);
    proof {
        assert(line@ =~= label() + digits@);
        assert forall|other: Seq<u8>| other.len() == 32 && line@ == #[trigger] seed_text(other) implies digits@
            == hex_text(other) by {
            assert(digits@ =~= seed_text(other).skip(12));
            assert(seed_text(other).skip(12) =~= hex_text(other));
        }
    }
    seed
}

pub fn write(text: &mut String, seed: &[u8; 32])
    requires
        old(text)@.len() == 0,
    ensures
        encode_utf8(final(text)@) == seed_text(seed@),
{
    text.push_str("post-seed-1:");
    hex::push(text, seed.as_slice());
    assert(encode_utf8(text@) == seed_text(seed@)) by {
        assert(text@ =~= "post-seed-1:"@ + hex_characters(seed@));
        lemma_the_bytes_of_a_written_seed(seed@);
    };
}

} // verus!

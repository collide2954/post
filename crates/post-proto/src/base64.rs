use vstd::prelude::*;

use crate::ascii::{is_digit, is_lower, is_upper};

verus! {

pub open spec fn symbol(value: int) -> u8 {
    if value < 26 {
        (b'A' + value) as u8
    } else if value < 52 {
        (b'a' + value - 26) as u8
    } else if value < 62 {
        (b'0' + value - 52) as u8
    } else if value == 62 {
        b'-'
    } else {
        b'_'
    }
}

pub open spec fn byte_at(bytes: Seq<u8>, index: int) -> int {
    if 0 <= index < bytes.len() {
        bytes[index] as int
    } else {
        0
    }
}

pub open spec fn sextet(bytes: Seq<u8>, index: int) -> int {
    let first = 3 * (index / 4);
    if index % 4 == 0 {
        byte_at(bytes, first) / 4
    } else if index % 4 == 1 {
        byte_at(bytes, first) % 4 * 16 + byte_at(bytes, first + 1) / 16
    } else if index % 4 == 2 {
        byte_at(bytes, first + 1) % 16 * 4 + byte_at(bytes, first + 2) / 64
    } else {
        byte_at(bytes, first + 2) % 64
    }
}

pub open spec fn text_length(bytes: int) -> int {
    (4 * bytes + 2) / 3
}

pub open spec fn base64_text(bytes: Seq<u8>) -> Seq<u8> {
    Seq::new(text_length(bytes.len() as int) as nat, |index: int| symbol(sextet(bytes, index)))
}

pub open spec fn is_symbol(byte: u8) -> bool {
    is_upper(byte) || is_lower(byte) || is_digit(byte) || byte == b'-' || byte == b'_'
}

pub open spec fn has_sextets(values: Seq<u8>, bytes: Seq<u8>) -> bool {
    &&& values.len() == text_length(bytes.len() as int)
    &&& forall|index: int|
        0 <= index < values.len() ==> values[index] as int == #[trigger] sextet(bytes, index)
}

proof fn lemma_a_symbol_gives_its_value(value: int, other: int)
    requires
        0 <= value < 64,
        0 <= other < 64,
    ensures
        is_symbol(symbol(value)),
        symbol(value) == symbol(other) ==> value == other,
{
}

proof fn lemma_a_sextet_is_small(bytes: Seq<u8>, index: int)
    requires
        0 <= index,
    ensures
        0 <= sextet(bytes, index) < 64,
{
}

proof fn lemma_a_sextet_reads_its_group(bytes: Seq<u8>, other: Seq<u8>, index: int)
    requires
        0 <= index,
        bytes.len() == other.len(),
        forall|i: int|
            3 * (index / 4) <= i < 3 * (index / 4) + 3 && i < bytes.len() ==> bytes[i] == other[i],
    ensures
        sextet(bytes, index) == sextet(other, index),
{
}

proof fn lemma_the_sextets_of_a_group(
    bytes: Seq<u8>,
    group: int,
    first: int,
    second: int,
    third: int,
    fourth: int,
)
    requires
        0 <= group,
        3 * group + 2 < bytes.len(),
        0 <= first < 64,
        0 <= second < 64,
        0 <= third < 64,
        0 <= fourth < 64,
        bytes[3 * group] == first * 4 + second / 16,
        bytes[3 * group + 1] == second % 16 * 16 + third / 4,
        bytes[3 * group + 2] == third % 4 * 64 + fourth,
    ensures
        sextet(bytes, 4 * group) == first,
        sextet(bytes, 4 * group + 1) == second,
        sextet(bytes, 4 * group + 2) == third,
        sextet(bytes, 4 * group + 3) == fourth,
{
    assert((4 * group) / 4 == group && (4 * group) % 4 == 0);
    assert((4 * group + 1) / 4 == group && (4 * group + 1) % 4 == 1);
    assert((4 * group + 2) / 4 == group && (4 * group + 2) % 4 == 2);
    assert((4 * group + 3) / 4 == group && (4 * group + 3) % 4 == 3);
}

proof fn lemma_the_length_of_a_text(bytes: int)
    requires
        0 <= bytes,
    ensures
        text_length(bytes) == 4 * (bytes / 3) + if bytes % 3 == 0 {
            0int
        } else {
            bytes % 3 + 1
        },
{
}

pub proof fn lemma_a_text_gives_its_bytes(bytes: Seq<u8>, other: Seq<u8>)
    requires
        bytes.len() == other.len(),
        base64_text(bytes) == base64_text(other),
    ensures
        bytes == other,
{
    let length = bytes.len() as int;
    lemma_the_length_of_a_text(length);
    assert forall|index: int| 0 <= index < text_length(length) implies sextet(bytes, index)
        == sextet(other, index) by {
        lemma_a_sextet_is_small(bytes, index);
        lemma_a_sextet_is_small(other, index);
        lemma_a_symbol_gives_its_value(sextet(bytes, index), sextet(other, index));
        assert(base64_text(bytes)[index] == symbol(sextet(bytes, index)));
        assert(base64_text(other)[index] == symbol(sextet(other, index)));
    }
    assert forall|i: int| 0 <= i < length implies bytes[i] == other[i] by {
        let group = i / 3;
        lemma_a_byte_from_its_sextets(bytes, i);
        lemma_a_byte_from_its_sextets(other, i);
        assert(4 * group + i % 3 + 1 < text_length(length));
        assert(sextet(bytes, 4 * group + i % 3) == sextet(other, 4 * group + i % 3));
        assert(sextet(bytes, 4 * group + i % 3 + 1) == sextet(other, 4 * group + i % 3 + 1));
    }
    assert(bytes =~= other);
}

proof fn lemma_a_byte_from_its_sextets(bytes: Seq<u8>, i: int)
    requires
        0 <= i < bytes.len(),
    ensures
        bytes[i] as int == if i % 3 == 0 {
            sextet(bytes, 4 * (i / 3)) * 4 + sextet(bytes, 4 * (i / 3) + 1) / 16
        } else if i % 3 == 1 {
            sextet(bytes, 4 * (i / 3) + 1) % 16 * 16 + sextet(bytes, 4 * (i / 3) + 2) / 4
        } else {
            sextet(bytes, 4 * (i / 3) + 2) % 4 * 64 + sextet(bytes, 4 * (i / 3) + 3)
        },
{
    let group = i / 3;
    assert((4 * group) / 4 == group && (4 * group) % 4 == 0);
    assert((4 * group + 1) / 4 == group && (4 * group + 1) % 4 == 1);
    assert((4 * group + 2) / 4 == group && (4 * group + 2) % 4 == 2);
    assert((4 * group + 3) / 4 == group && (4 * group + 3) % 4 == 3);
}

fn symbol_of(value: u8) -> (character: u8)
    requires
        value < 64,
    ensures
        character == symbol(value as int),
{
    if value < 26 {
        b'A' + value
    } else if value < 52 {
        b'a' + (value - 26)
    } else if value < 62 {
        b'0' + (value - 52)
    } else if value == 62 {
        b'-'
    } else {
        b'_'
    }
}

pub fn encode(bytes: &[u8]) -> (text: Vec<u8>)
    ensures
        text@ == base64_text(bytes@),
{
    let mut text = Vec::new();
    let groups = bytes.len() / 3;
    let mut group = 0;
    while group < groups
        invariant
            group <= groups == bytes@.len() / 3,
            text@.len() == 4 * group,
            forall|index: int|
                0 <= index < text@.len() ==> text@[index] == symbol(#[trigger] sextet(bytes@, index)),
        decreases groups - group,
    {
        let first = bytes[3 * group];
        let second = bytes[3 * group + 1];
        let third = bytes[3 * group + 2];
        text.push(symbol_of(first / 4));
        text.push(symbol_of(first % 4 * 16 + second / 16));
        text.push(symbol_of(second % 16 * 4 + third / 64));
        text.push(symbol_of(third % 64));
        group += 1;
    }
    if bytes.len() % 3 == 1 {
        let first = bytes[3 * groups];
        text.push(symbol_of(first / 4));
        text.push(symbol_of(first % 4 * 16));
    } else if bytes.len() % 3 == 2 {
        let first = bytes[3 * groups];
        let second = bytes[3 * groups + 1];
        text.push(symbol_of(first / 4));
        text.push(symbol_of(first % 4 * 16 + second / 16));
        text.push(symbol_of(second % 16 * 4));
    }
    assert(text@ =~= base64_text(bytes@));
    text
}

fn value_of(character: u8) -> (value: Option<u8>)
    ensures
        value matches Some(value) ==> value < 64 && character == symbol(value as int),
        value is None ==> !is_symbol(character),
{
    if is_upper(character) {
        Some(character - b'A')
    } else if is_lower(character) {
        Some(character - b'a' + 26)
    } else if is_digit(character) {
        Some(character - b'0' + 52)
    } else if character == b'-' {
        Some(62)
    } else if character == b'_' {
        Some(63)
    } else {
        None
    }
}

fn values_of(text: &[u8]) -> (values: Option<Vec<u8>>)
    ensures
        values matches Some(values) ==> values@.len() == text@.len() && forall|index: int|
            0 <= index < text@.len() ==> #[trigger] values@[index] < 64 && text@[index] == symbol(
                values@[index] as int,
            ),
        values is None ==> exists|index: int|
            0 <= index < text@.len() && !is_symbol(#[trigger] text@[index]),
{
    let mut values: Vec<u8> = Vec::with_capacity(text.len());
    let mut index = 0;
    while index < text.len()
        invariant
            index <= text@.len(),
            values@.len() == index,
            forall|i: int|
                0 <= i < index ==> #[trigger] values@[i] < 64 && text@[i] == symbol(values@[i] as int),
        decreases text@.len() - index,
    {
        if let Some(value) = value_of(text[index]) {
            values.push(value);
        } else {
            assert(!is_symbol(text@[index as int]));
            return None;
        }
        index += 1;
    }
    Some(values)
}

fn bytes_of<const N: usize>(values: &[u8]) -> (bytes: Option<[u8; N]>)
    requires
        forall|index: int| 0 <= index < values@.len() ==> #[trigger] values@[index] < 64,
    ensures
        bytes matches Some(bytes) ==> has_sextets(values@, bytes@),
        bytes is None ==> forall|bytes: Seq<u8>|
            bytes.len() == N ==> !#[trigger] has_sextets(values@, bytes),
{
    let groups = N / 3;
    let rest = N % 3;
    let tail = if rest == 0 {
        0
    } else {
        rest + 1
    };
    proof {
        lemma_the_length_of_a_text(N as int);
    }
    if values.len() / 4 != groups || values.len() % 4 != tail {
        return None;
    }
    let mut bytes = [0; N];
    let mut group = 0;
    while group < groups
        invariant
            group <= groups == N / 3,
            values@.len() == text_length(N as int) == 4 * groups + tail,
            bytes@.len() == N,
            forall|index: int| 0 <= index < values@.len() ==> #[trigger] values@[index] < 64,
            forall|index: int|
                0 <= index < 4 * group ==> values@[index] as int == #[trigger] sextet(bytes@, index),
        decreases groups - group,
    {
        let first = values[4 * group];
        let second = values[4 * group + 1];
        let third = values[4 * group + 2];
        let fourth = values[4 * group + 3];
        let ghost before = bytes@;
        bytes[3 * group] = first * 4 + second / 16;
        bytes[3 * group + 1] = second % 16 * 16 + third / 4;
        bytes[3 * group + 2] = third % 4 * 64 + fourth;
        proof {
            assert forall|index: int| 0 <= index < 4 * group implies values@[index] as int
                == #[trigger] sextet(bytes@, index) by {
                lemma_a_sextet_reads_its_group(before, bytes@, index);
            }
            lemma_the_sextets_of_a_group(
                bytes@,
                group as int,
                first as int,
                second as int,
                third as int,
                fourth as int,
            );
        }
        group += 1;
    }
    let ghost before = bytes@;
    if rest == 1 {
        let first = values[4 * groups];
        let second = values[4 * groups + 1];
        if !second.is_multiple_of(16) {
            assert forall|other: Seq<u8>| other.len() == N implies !#[trigger] has_sextets(
                values@,
                other,
            ) by {
                assert(sextet(other, 4 * groups + 1) % 16 == 0);
            }
            return None;
        }
        bytes[3 * groups] = first * 4 + second / 16;
        assert(sextet(bytes@, 4 * groups as int) == first);
        assert(sextet(bytes@, 4 * groups + 1) == second);
    } else if rest == 2 {
        let first = values[4 * groups];
        let second = values[4 * groups + 1];
        let third = values[4 * groups + 2];
        if !third.is_multiple_of(4) {
            assert forall|other: Seq<u8>| other.len() == N implies !#[trigger] has_sextets(
                values@,
                other,
            ) by {
                assert(sextet(other, 4 * groups + 2) % 4 == 0);
            }
            return None;
        }
        bytes[3 * groups] = first * 4 + second / 16;
        bytes[3 * groups + 1] = second % 16 * 16 + third / 4;
        assert(sextet(bytes@, 4 * groups as int) == first);
        assert(sextet(bytes@, 4 * groups + 1) == second);
        assert(sextet(bytes@, 4 * groups + 2) == third);
    }
    proof {
        assert forall|index: int| 0 <= index < 4 * groups implies values@[index] as int
            == #[trigger] sextet(bytes@, index) by {
            lemma_a_sextet_reads_its_group(before, bytes@, index);
        }
    }
    Some(bytes)
}

pub fn decode<const N: usize>(text: &[u8]) -> (bytes: Option<[u8; N]>)
    ensures
        bytes matches Some(bytes) ==> text@ == base64_text(bytes@),
        bytes is None ==> forall|bytes: Seq<u8>|
            bytes.len() == N ==> text@ != #[trigger] base64_text(bytes),
{
    match values_of(text) {
        None => {
            assert forall|other: Seq<u8>| other.len() == N implies text@ != #[trigger] base64_text(
                other,
            ) by {
                let index = choose|index: int|
                    0 <= index < text@.len() && !is_symbol(#[trigger] text@[index]);
                lemma_a_sextet_is_small(other, index);
                lemma_a_symbol_gives_its_value(sextet(other, index), 0);
                if text@.len() == base64_text(other).len() {
                    assert(base64_text(other)[index] == symbol(sextet(other, index)));
                }
            }
            None
        },
        Some(values) => {
            let bytes = bytes_of::<N>(values.as_slice());
            proof {
                if let Some(bytes) = bytes {
                    assert(text@ =~= base64_text(bytes@));
                } else {
                    assert forall|other: Seq<u8>| other.len() == N implies text@
                        != #[trigger] base64_text(other) by {
                        if text@ == base64_text(other) {
                            assert forall|index: int| 0 <= index < values@.len() implies values@[index] as int
                                == #[trigger] sextet(other, index) by {
                                lemma_a_sextet_is_small(other, index);
                                lemma_a_symbol_gives_its_value(
                                    values@[index] as int,
                                    sextet(other, index),
                                );
                                assert(base64_text(other)[index] == symbol(sextet(other, index)));
                            }
                            assert(has_sextets(values@, other));
                        }
                    }
                }
            }
            bytes
        },
    }
}

} // verus!

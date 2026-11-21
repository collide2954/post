use vstd::prelude::*;

verus! {

pub open spec fn spec_is_lower(byte: u8) -> bool {
    b'a' <= byte <= b'z'
}

pub open spec fn spec_is_digit(byte: u8) -> bool {
    b'0' <= byte <= b'9'
}

#[expect(clippy::manual_is_ascii_check, reason = "Verus has no specification for is_ascii_lowercase")]
#[verifier::when_used_as_spec(spec_is_lower)]
pub fn is_lower(byte: u8) -> (lower: bool)
    ensures
        lower == spec_is_lower(byte),
{
    (b'a'..=b'z').contains(&byte)
}

#[expect(clippy::manual_is_ascii_check, reason = "Verus has no specification for is_ascii_digit")]
#[verifier::when_used_as_spec(spec_is_digit)]
pub fn is_digit(byte: u8) -> (digit: bool)
    ensures
        digit == spec_is_digit(byte),
{
    (b'0'..=b'9').contains(&byte)
}

} // verus!

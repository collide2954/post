use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::string::StringSliceAdditionalSpecFns;
#[cfg(verus_keep_ghost)]
use vstd::utf8::encode_utf8;

use crate::ascii::{is_digit, is_lower, is_upper};

verus! {

pub open spec fn is_code_character(byte: u8) -> bool {
    is_upper(byte) || is_lower(byte) || is_digit(byte)
}

pub open spec fn is_code(bytes: Seq<u8>) -> bool {
    &&& bytes.len() == 12
    &&& forall|i: int| 0 <= i < bytes.len() ==> is_code_character(#[trigger] bytes[i])
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Code(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeError;

impl Code {
    pub const LENGTH: usize = 12;

    #[verifier::type_invariant]
    spec fn is_a_code(self) -> bool {
        is_code(encode_utf8(self.0@))
    }

    pub closed spec fn view(self) -> Seq<char> {
        self.0@
    }

    pub fn parse(text: &str) -> (code: Result<Self, CodeError>)
        ensures
            code is Ok <==> is_code(text.spec_bytes()),
            code matches Ok(code) ==> code@ == text@,
    {
        if has_the_form(text.as_bytes()) {
            Ok(Self(text.to_owned()))
        } else {
            Err(CodeError)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> (text: &str)
        ensures
            text@ == self@,
            is_code(text.spec_bytes()),
    {
        proof {
            use_type_invariant(self);
        }
        self.0.as_str()
    }
}

impl Clone for Code {
    fn clone(&self) -> (code: Self)
        ensures
            code@ == self@,
    {
        proof {
            use_type_invariant(self);
        }
        Self(self.0.clone())
    }
}

fn has_the_form(bytes: &[u8]) -> (form: bool)
    ensures
        form == is_code(bytes@),
{
    if bytes.len() != Code::LENGTH {
        return false;
    }
    let mut index = 0;
    while index < bytes.len()
        invariant
            index <= bytes.len(),
            forall|i: int| 0 <= i < index ==> is_code_character(#[trigger] bytes@[i]),
        decreases bytes.len() - index,
    {
        let byte = bytes[index];
        if !(is_upper(byte) || is_lower(byte) || is_digit(byte)) {
            return false;
        }
        index += 1;
    }
    true
}

} // verus!

impl Code {
    pub const ALPHABET: &[u8; 62] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
}
impl fmt::Display for Code {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Display for CodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a code must have {} letters and digits",
            Code::LENGTH
        )
    }
}

impl std::error::Error for CodeError {}

impl JsonSchema for Code {
    fn schema_name() -> Cow<'static, str> {
        "Code".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "The code of an invitation: 12 letters and digits.",
            "type": "string",
            "pattern": "^[A-Za-z0-9]{12}$"
        })
    }
}

impl Serialize for Code {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Code {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{Code, CodeError};

    #[test]
    fn accepts_twelve_letters_and_digits() {
        for valid in [
            "7Hq2kdX9mPq4",
            "AAAAAAAAAAAA",
            "zzzzzzzzzzzz",
            "000000000000",
        ] {
            let code = Code::parse(valid).expect(valid);
            assert_eq!(code.as_str(), valid);
            assert_eq!(code.to_string(), valid);
        }
    }

    #[test]
    fn refuses_an_incorrect_length() {
        for invalid in ["", "7Hq2kd", "7Hq2kdX9mPq", "7Hq2kdX9mPq4a"] {
            assert_eq!(Code::parse(invalid), Err(CodeError), "{invalid:?}");
        }
    }

    #[test]
    fn refuses_a_character_that_is_not_a_letter_or_a_digit() {
        for invalid in [
            "7Hq2kdX9mPq-",
            "7Hq2kdX9mPq_",
            "7Hq2kdX9mPq ",
            "7Hq2kdX9mPq/",
            "7Hq2kdX9mP\u{e9}",
        ] {
            assert_eq!(Code::parse(invalid), Err(CodeError), "{invalid:?}");
        }
    }

    #[test]
    fn has_an_alphabet_of_all_the_characters_that_it_accepts() {
        assert_eq!(Code::ALPHABET.len(), 62);
        let mut sorted = Code::ALPHABET.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 62);
        assert!(sorted.iter().all(u8::is_ascii_alphanumeric));
    }

    #[test]
    fn is_a_json_string() {
        let code = Code::parse("7Hq2kdX9mPq4").expect("valid");
        let json = serde_json::to_string(&code).expect("a code becomes JSON");
        assert_eq!(json, "\"7Hq2kdX9mPq4\"");
        assert_eq!(serde_json::from_str::<Code>(&json).expect("valid"), code);
        assert!(serde_json::from_str::<Code>("\"short\"").is_err());
    }

    #[test]
    fn describes_the_error() {
        assert_eq!(
            CodeError.to_string(),
            "a code must have 12 letters and digits"
        );
    }
}

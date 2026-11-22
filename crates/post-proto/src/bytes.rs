use std::borrow::Cow;
use std::fmt::{self, Write as _};

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::base64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BytesError;

impl fmt::Display for BytesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the text is not base64url text of the correct length")
    }
}

impl std::error::Error for BytesError {}

macro_rules! byte_text {
    ($name:ident, $length:literal, $pattern:literal, $description:literal) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name([u8; $length]);

        impl $name {
            pub const BYTES: usize = $length;

            #[must_use]
            pub const fn from_bytes(bytes: [u8; $length]) -> Self {
                Self(bytes)
            }

            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; $length] {
                &self.0
            }

            pub fn parse(text: &str) -> Result<Self, BytesError> {
                base64::decode(text.as_bytes())
                    .map(Self)
                    .ok_or(BytesError)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                base64::encode(&self.0)
                    .into_iter()
                    .try_for_each(|byte| formatter.write_char(char::from(byte)))
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, concat!(stringify!($name), "({})"), self)
            }
        }

        impl JsonSchema for $name {
            fn schema_name() -> Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
                json_schema!({
                    "description": $description,
                    "type": "string",
                    "pattern": $pattern
                })
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = String::deserialize(deserializer)?;
                Self::parse(&text).map_err(D::Error::custom)
            }
        }
    };
}

byte_text!(
    Key,
    32,
    "^[A-Za-z0-9_-]{43}$",
    "A public key: 32 bytes as base64url text with no `=` characters."
);
byte_text!(
    Nonce,
    32,
    "^[A-Za-z0-9_-]{43}$",
    "A random value: 32 bytes as base64url text with no `=` characters."
);
byte_text!(
    Signature,
    64,
    "^[A-Za-z0-9_-]{86}$",
    "A signature: 64 bytes as base64url text with no `=` characters."
);

#[cfg(test)]
mod tests {
    use super::{BytesError, Key, Nonce, Signature};

    fn counted<const N: usize>() -> [u8; N] {
        std::array::from_fn(|index| u8::try_from(index).expect("a small index"))
    }

    #[test]
    fn writes_bytes_as_base64url_text() {
        assert_eq!(Key::from_bytes([0; 32]).to_string(), "A".repeat(43));
        assert_eq!(
            Nonce::from_bytes([0xfb; 32]).to_string(),
            format!("{}-_s", "-_v7".repeat(10))
        );
        assert_eq!(Signature::from_bytes([0; 64]).to_string(), "A".repeat(86));
        assert_eq!(
            Key::from_bytes(counted()).to_string(),
            "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8"
        );
    }

    #[test]
    fn reads_the_text_that_it_writes() {
        let key = Key::from_bytes(counted());
        assert_eq!(Key::parse(&key.to_string()), Ok(key));
        assert_eq!(key.as_bytes(), &counted::<32>());

        let nonce = Nonce::from_bytes([0xfb; 32]);
        assert_eq!(Nonce::parse(&nonce.to_string()), Ok(nonce));

        let signature = Signature::from_bytes(counted());
        assert_eq!(Signature::parse(&signature.to_string()), Ok(signature));
        assert_eq!(signature.as_bytes(), &counted::<64>());
    }

    #[test]
    fn gives_the_number_of_bytes() {
        assert_eq!(Key::BYTES, 32);
        assert_eq!(Nonce::BYTES, 32);
        assert_eq!(Signature::BYTES, 64);
    }

    #[test]
    fn refuses_a_text_with_an_incorrect_length() {
        for length in [0, 1, 42, 44, 86] {
            assert_eq!(Key::parse(&"A".repeat(length)), Err(BytesError), "{length}");
        }
        for length in [0, 43, 85, 87, 88] {
            assert_eq!(
                Signature::parse(&"A".repeat(length)),
                Err(BytesError),
                "{length}"
            );
        }
    }

    #[test]
    fn refuses_a_character_that_is_not_in_the_alphabet() {
        let valid = "A".repeat(42);
        for character in ["+", "/", "=", " ", "\n", "."] {
            let text = format!("{valid}{character}");
            assert_eq!(Key::parse(&text), Err(BytesError), "{character:?}");
        }
        let text = format!("{}\u{e9}", "A".repeat(41));
        assert_eq!(text.len(), 43);
        assert_eq!(Key::parse(&text), Err(BytesError));
    }

    #[test]
    fn refuses_a_second_text_for_the_same_bytes() {
        let text = format!("{}B", "A".repeat(42));

        assert_eq!(Key::parse(&text), Err(BytesError));
    }

    #[test]
    fn shows_its_text_for_a_developer() {
        let key = Key::from_bytes([0; 32]);
        assert_eq!(format!("{key:?}"), format!("Key({})", "A".repeat(43)));

        let signature = Signature::from_bytes([0; 64]);
        assert_eq!(
            format!("{signature:?}"),
            format!("Signature({})", "A".repeat(86))
        );
    }

    #[test]
    fn is_a_json_string() {
        let key = Key::from_bytes(counted());
        let json = serde_json::to_string(&key).expect("a key becomes JSON");
        assert_eq!(json, "\"AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8\"");
        assert_eq!(serde_json::from_str::<Key>(&json).expect("valid"), key);

        assert!(serde_json::from_str::<Key>("\"AAAA\"").is_err());
        assert!(serde_json::from_str::<Key>("7").is_err());
    }

    #[test]
    fn describes_the_error() {
        assert_eq!(
            BytesError.to_string(),
            "the text is not base64url text of the correct length"
        );
    }
}

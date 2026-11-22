use std::fmt;

use sha2::{Digest, Sha256};

use crate::hex;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint([u8; Fingerprint::BYTES]);

impl Fingerprint {
    pub const BYTES: usize = 32;

    #[must_use]
    pub fn of_certificate(der: &[u8]) -> Self {
        Self(Sha256::digest(der).into())
    }

    #[must_use]
    pub const fn from_bytes(bytes: [u8; Self::BYTES]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        hex::decode(text.as_bytes()).map(Self)
    }

    #[must_use]
    pub fn grouped(&self) -> String {
        let digits = self.to_string();
        let mut grouped = String::with_capacity(digits.len() + digits.len() / 4);
        for (index, digit) in digits.chars().enumerate() {
            if index > 0 && index % 4 == 0 {
                grouped.push(' ');
            }
            grouped.push(digit);
        }
        grouped
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut text = String::with_capacity(2 * Self::BYTES);
        hex::push(&mut text, &self.0);
        formatter.write_str(&text)
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Fingerprint({self})")
    }
}

#[cfg(test)]
mod tests {
    use super::Fingerprint;

    const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn is_the_sha_256_hash_of_the_certificate() {
        assert_eq!(Fingerprint::of_certificate(b"").to_string(), EMPTY);
        assert_eq!(Fingerprint::of_certificate(b"abc").to_string(), ABC);
    }

    #[test]
    fn reads_the_text_that_it_writes() {
        let fingerprint = Fingerprint::of_certificate(b"abc");

        assert_eq!(Fingerprint::parse(ABC), Some(fingerprint));
        assert_eq!(
            Fingerprint::parse(&fingerprint.to_string()),
            Some(fingerprint)
        );
        assert_eq!(
            Fingerprint::from_bytes([0xab; 32]).to_string(),
            "ab".repeat(32)
        );
    }

    #[test]
    fn refuses_a_text_that_is_not_a_fingerprint() {
        for text in [
            "",
            "ab",
            &ABC[2..],
            &format!("{ABC}00"),
            &ABC.to_uppercase(),
            &ABC.replace('b', "g"),
            &format!(" {ABC}"),
        ] {
            assert_eq!(Fingerprint::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn shows_groups_of_four_characters_to_a_person() {
        let grouped = Fingerprint::of_certificate(b"abc").grouped();

        assert_eq!(
            grouped,
            "ba78 16bf 8f01 cfea 4141 40de 5dae 2223 b003 61a3 9617 7a9c b410 ff61 f200 15ad"
        );
    }

    #[test]
    fn shows_its_text_to_a_developer() {
        let fingerprint = Fingerprint::of_certificate(b"abc");

        assert_eq!(format!("{fingerprint:?}"), format!("Fingerprint({ABC})"));
    }
}

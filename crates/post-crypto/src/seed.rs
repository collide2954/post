use std::fmt;

use hkdf::Hkdf;
use post_proto::Key;
use sha2::Sha256;
use zeroize::{Zeroize as _, Zeroizing};

use crate::keys::AccountKey;
use crate::random;

mod text;

const ACCOUNT_KEY: &[u8] = b"post-account-key-1\0";

pub struct Seed([u8; Seed::BYTES]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeedError;

impl Seed {
    pub const BYTES: usize = 32;

    #[must_use]
    pub fn generate() -> Self {
        Self(random::bytes())
    }

    #[must_use]
    pub const fn from_bytes(bytes: [u8; Self::BYTES]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub fn encode(&self) -> Zeroizing<String> {
        let mut text = Zeroizing::new(String::with_capacity(text::LENGTH));
        text::write(&mut text, &self.0);
        text
    }

    pub fn decode(text: &str) -> Result<Self, SeedError> {
        text::read(text).map(Self).ok_or(SeedError)
    }

    #[must_use]
    pub fn account_key(&self, server: &Key) -> AccountKey {
        let info = [ACCOUNT_KEY, server.as_bytes().as_slice()].concat();
        let mut secret = Zeroizing::new([0; 32]);
        Hkdf::<Sha256>::new(None, &self.0)
            .expand(&info, secret.as_mut_slice())
            .expect("HKDF with SHA-256 can give 32 bytes");
        AccountKey::from_bytes(&secret)
    }
}

impl Clone for Seed {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl Drop for Seed {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for Seed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Seed(..)")
    }
}

impl fmt::Display for SeedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the text is not a seed of Post")
    }
}

impl std::error::Error for SeedError {}

#[cfg(test)]
mod tests {
    use post_proto::Key;

    use super::{Seed, SeedError};
    use crate::ServerKey;
    use crate::hex;

    const TEXT: &str =
        "post-seed-1:000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    fn counted() -> Seed {
        Seed::from_bytes(std::array::from_fn(|index| {
            u8::try_from(index).expect("a small index")
        }))
    }

    fn server() -> Key {
        let public = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
        Key::from_bytes(hex::decode(public.as_bytes()).expect("valid hexadecimal text"))
    }

    #[test]
    fn makes_a_copy_that_gives_the_same_account_key() {
        let seed = counted();

        let copy = seed.clone();

        assert_eq!(
            copy.account_key(&server()).public(),
            seed.account_key(&server()).public()
        );
    }

    #[test]
    fn writes_a_seed_as_one_line_of_text() {
        assert_eq!(counted().encode().as_str(), TEXT);
    }

    #[test]
    fn reads_the_text_that_it_writes() {
        let seed = Seed::decode(TEXT).expect("a valid seed");

        assert_eq!(seed.encode().as_str(), TEXT);
    }

    #[test]
    fn accepts_one_line_break_after_the_seed() {
        for end in ["\n", "\r\n"] {
            let seed = Seed::decode(&format!("{TEXT}{end}")).expect("a valid seed");
            assert_eq!(seed.encode().as_str(), TEXT, "{end:?}");
        }
    }

    #[test]
    fn refuses_a_text_that_is_not_a_seed() {
        let digits = &TEXT["post-seed-1:".len()..];
        let texts = [
            String::new(),
            digits.to_owned(),
            format!("post-seed-2:{digits}"),
            format!("POST-SEED-1:{digits}"),
            format!("post-seed-1:{}", &digits[2..]),
            format!("post-seed-1:{digits}00"),
            format!("post-seed-1:{}", digits.to_uppercase()),
            format!(" {TEXT}"),
            format!("{TEXT} "),
            format!("{TEXT}\n\n"),
            format!("{TEXT}\r"),
        ];
        for text in texts {
            assert_eq!(Seed::decode(&text).err(), Some(SeedError), "{text:?}");
        }
    }

    #[test]
    fn makes_a_different_seed_each_time() {
        assert_ne!(Seed::generate().encode(), Seed::generate().encode());
    }

    #[test]
    fn calculates_the_account_key_with_hkdf() {
        let expected = "ad9e14c9f86cbdd79ca7314be65c336c6cd0ed7ddf24dd51e3ca397bfee85443";
        let expected =
            ServerKey::from_bytes(&hex::decode(expected.as_bytes()).expect("valid text"));

        let account = counted().account_key(&server());

        assert_eq!(account.public(), expected.public());
    }

    #[test]
    fn gives_the_same_account_key_for_the_same_server() {
        let seed = counted();

        assert_eq!(
            seed.account_key(&server()).public(),
            seed.account_key(&server()).public()
        );
    }

    #[test]
    fn gives_a_different_account_key_for_each_server() {
        let seed = counted();
        let other = ServerKey::from_bytes(&[9; 32]).public();

        assert_ne!(
            seed.account_key(&server()).public(),
            seed.account_key(&other).public()
        );
    }

    #[test]
    fn gives_a_different_account_key_for_each_seed() {
        let other = Seed::from_bytes([9; 32]);

        assert_ne!(
            counted().account_key(&server()).public(),
            other.account_key(&server()).public()
        );
    }

    #[test]
    fn does_not_show_the_secret_to_a_developer() {
        assert_eq!(format!("{:?}", counted()), "Seed(..)");
    }

    #[test]
    fn describes_the_error() {
        assert_eq!(SeedError.to_string(), "the text is not a seed of Post");
    }
}

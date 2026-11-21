use std::fmt;

use ed25519_dalek::{Signer as _, SigningKey, VerifyingKey};
use post_proto::{Key, Nonce, Signature};
use zeroize::Zeroizing;

use crate::random;

const SERVER_PROOF: &[u8] = b"post-server-proof-0\0";
const ACCOUNT_PROOF: &[u8] = b"post-account-proof-0\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exchange<'a> {
    pub address: &'a str,
    pub client: &'a Nonce,
    pub server: &'a Nonce,
}

pub struct ServerKey(SigningKey);

pub struct AccountKey(SigningKey);

impl Exchange<'_> {
    #[must_use]
    pub fn server_proof_is_valid(&self, server: &Key, proof: &Signature) -> bool {
        is_valid(server, &self.server_message(), proof)
    }

    #[must_use]
    pub fn account_proof_is_valid(&self, account: &Key, server: &Key, proof: &Signature) -> bool {
        is_valid(account, &self.account_message(server), proof)
    }

    fn server_message(&self) -> Vec<u8> {
        let parts: [&[u8]; 5] = [
            SERVER_PROOF,
            self.address.as_bytes(),
            &[0],
            self.client.as_bytes(),
            self.server.as_bytes(),
        ];
        parts.concat()
    }

    fn account_message(&self, server: &Key) -> Vec<u8> {
        let parts: [&[u8]; 6] = [
            ACCOUNT_PROOF,
            self.address.as_bytes(),
            &[0],
            server.as_bytes(),
            self.client.as_bytes(),
            self.server.as_bytes(),
        ];
        parts.concat()
    }
}

fn is_valid(key: &Key, message: &[u8], proof: &Signature) -> bool {
    let Ok(key) = VerifyingKey::from_bytes(key.as_bytes()) else {
        return false;
    };
    let proof = ed25519_dalek::Signature::from_bytes(proof.as_bytes());
    key.verify_strict(message, &proof).is_ok()
}

fn sign(key: &SigningKey, message: &[u8]) -> Signature {
    Signature::from_bytes(key.sign(message).to_bytes())
}

fn public(key: &SigningKey) -> Key {
    Key::from_bytes(key.verifying_key().to_bytes())
}

impl ServerKey {
    #[must_use]
    pub fn generate() -> Self {
        Self::from_bytes(&Zeroizing::new(random::bytes()))
    }

    #[must_use]
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self(SigningKey::from_bytes(bytes))
    }

    #[must_use]
    pub fn to_bytes(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.0.to_bytes())
    }

    #[must_use]
    pub fn public(&self) -> Key {
        public(&self.0)
    }

    #[must_use]
    pub fn prove(&self, exchange: &Exchange<'_>) -> Signature {
        sign(&self.0, &exchange.server_message())
    }
}

impl AccountKey {
    pub(crate) fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self(SigningKey::from_bytes(bytes))
    }

    #[must_use]
    pub fn public(&self) -> Key {
        public(&self.0)
    }

    #[must_use]
    pub fn prove(&self, server: &Key, exchange: &Exchange<'_>) -> Signature {
        sign(&self.0, &exchange.account_message(server))
    }
}

impl fmt::Debug for ServerKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ServerKey({})", self.public())
    }
}

impl fmt::Debug for AccountKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "AccountKey({})", self.public())
    }
}

#[cfg(test)]
mod tests {
    use post_proto::{Key, Nonce, Signature};

    use super::{AccountKey, Exchange, ServerKey};
    use crate::hex;

    const RFC_8032_SECRET: &str =
        "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
    const RFC_8032_PUBLIC: &str =
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";

    const CLIENT: Nonce = Nonce::from_bytes([1; 32]);
    const SERVER: Nonce = Nonce::from_bytes([2; 32]);
    const EXCHANGE: Exchange<'static> = Exchange {
        address: "chat.example.org:443",
        client: &CLIENT,
        server: &SERVER,
    };

    fn bytes(text: &str) -> [u8; 32] {
        hex::decode(text).expect("valid hexadecimal text")
    }

    fn server_key() -> ServerKey {
        ServerKey::from_bytes(&bytes(RFC_8032_SECRET))
    }

    fn account_key() -> AccountKey {
        AccountKey::from_bytes(&[7; 32])
    }

    fn changed(proof: &Signature, index: usize) -> Signature {
        let mut bytes = *proof.as_bytes();
        bytes[index] ^= 1;
        Signature::from_bytes(bytes)
    }

    #[test]
    fn gives_the_public_key_of_the_standard() {
        assert_eq!(
            server_key().public(),
            Key::from_bytes(bytes(RFC_8032_PUBLIC))
        );
    }

    #[test]
    fn gives_its_secret_bytes_for_storage() {
        let key = server_key();

        assert_eq!(*key.to_bytes(), bytes(RFC_8032_SECRET));
        assert_eq!(
            ServerKey::from_bytes(&key.to_bytes()).public(),
            key.public()
        );
    }

    #[test]
    fn makes_a_different_server_key_each_time() {
        assert_ne!(
            ServerKey::generate().public(),
            ServerKey::generate().public()
        );
    }

    #[test]
    fn makes_a_server_proof_that_is_valid_for_its_exchange() {
        let key = server_key();

        let proof = key.prove(&EXCHANGE);

        assert!(EXCHANGE.server_proof_is_valid(&key.public(), &proof));
    }

    #[test]
    fn makes_the_same_server_proof_as_the_first_release() {
        let proof = server_key().prove(&EXCHANGE);

        assert_eq!(
            proof.to_string(),
            "ZeA2ResstAfHtxRGEs6knEtI14hfp-IrwWCnVdDajcWimrtB6_yXo33nRHzuebREmPcb2GQo1XPcZONrCBKXCw"
        );
    }

    #[test]
    fn refuses_a_server_proof_for_a_different_exchange() {
        let key = server_key();
        let proof = key.prove(&EXCHANGE);
        let other = Nonce::from_bytes([3; 32]);

        let exchanges = [
            Exchange {
                address: "chat.example.org:444",
                ..EXCHANGE
            },
            Exchange {
                address: "",
                ..EXCHANGE
            },
            Exchange {
                client: &other,
                ..EXCHANGE
            },
            Exchange {
                server: &other,
                ..EXCHANGE
            },
            Exchange {
                client: &SERVER,
                server: &CLIENT,
                ..EXCHANGE
            },
        ];
        for exchange in exchanges {
            assert!(
                !exchange.server_proof_is_valid(&key.public(), &proof),
                "{exchange:?}"
            );
        }
    }

    #[test]
    fn refuses_a_server_proof_from_a_different_key() {
        let proof = server_key().prove(&EXCHANGE);
        let other = ServerKey::from_bytes(&[9; 32]);

        assert!(!EXCHANGE.server_proof_is_valid(&other.public(), &proof));
    }

    #[test]
    fn refuses_a_proof_that_has_a_changed_bit() {
        let key = server_key();
        let proof = key.prove(&EXCHANGE);

        for index in [0, 31, 32, 63] {
            let proof = changed(&proof, index);
            assert!(
                !EXCHANGE.server_proof_is_valid(&key.public(), &proof),
                "{index}"
            );
        }
    }

    #[test]
    fn refuses_a_key_that_is_not_safe() {
        let proof = server_key().prove(&EXCHANGE);

        for key in [[0; 32], [0xff; 32]] {
            let key = Key::from_bytes(key);
            assert!(!EXCHANGE.server_proof_is_valid(&key, &proof), "{key:?}");
            assert!(
                !EXCHANGE.account_proof_is_valid(&key, &server_key().public(), &proof),
                "{key:?}"
            );
        }
    }

    #[test]
    fn makes_an_account_proof_that_is_valid_for_its_exchange() {
        let account = account_key();
        let server = server_key().public();

        let proof = account.prove(&server, &EXCHANGE);

        assert!(EXCHANGE.account_proof_is_valid(&account.public(), &server, &proof));
    }

    #[test]
    fn makes_the_same_account_proof_as_the_first_release() {
        let proof = account_key().prove(&server_key().public(), &EXCHANGE);

        assert_eq!(
            proof.to_string(),
            "BjlVxNBiIIeX1bj_mMJEDil5HcZEkS4Tpiv0yGGgFzrD_G4O9iuizC2NtPjI8LPJeBo6ftkL9yyYiPCHIgaYCA"
        );
    }

    #[test]
    fn refuses_an_account_proof_for_a_different_exchange_or_server() {
        let account = account_key();
        let server = server_key().public();
        let proof = account.prove(&server, &EXCHANGE);
        let other = Nonce::from_bytes([3; 32]);

        let exchanges = [
            Exchange {
                address: "false.example.org:443",
                ..EXCHANGE
            },
            Exchange {
                client: &other,
                ..EXCHANGE
            },
            Exchange {
                server: &other,
                ..EXCHANGE
            },
        ];
        for exchange in exchanges {
            assert!(
                !exchange.account_proof_is_valid(&account.public(), &server, &proof),
                "{exchange:?}"
            );
        }

        let other_server = ServerKey::from_bytes(&[9; 32]).public();
        assert!(!EXCHANGE.account_proof_is_valid(&account.public(), &other_server, &proof));

        let other_account = AccountKey::from_bytes(&[8; 32]).public();
        assert!(!EXCHANGE.account_proof_is_valid(&other_account, &server, &proof));
    }

    #[test]
    fn keeps_the_two_kinds_of_proof_apart() {
        let key = server_key();
        let same_key = AccountKey::from_bytes(&bytes(RFC_8032_SECRET));
        assert_eq!(same_key.public(), key.public());

        let server_proof = key.prove(&EXCHANGE);
        let account_proof = same_key.prove(&key.public(), &EXCHANGE);

        assert!(!EXCHANGE.account_proof_is_valid(&key.public(), &key.public(), &server_proof));
        assert!(!EXCHANGE.server_proof_is_valid(&key.public(), &account_proof));
    }

    #[test]
    fn does_not_show_a_secret_to_a_developer() {
        let server = server_key();
        assert_eq!(
            format!("{server:?}"),
            format!("ServerKey({})", server.public())
        );

        let account = account_key();
        assert_eq!(
            format!("{account:?}"),
            format!("AccountKey({})", account.public())
        );
    }
}

use post_proto::Nonce;

pub(crate) fn bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).expect("the operating system gives random bytes");
    bytes
}

#[must_use]
pub fn nonce() -> Nonce {
    Nonce::from_bytes(bytes())
}

#[cfg(test)]
mod tests {
    use super::{bytes, nonce};

    #[test]
    fn gives_different_bytes_each_time() {
        assert_ne!(bytes::<32>(), bytes::<32>());
        assert_ne!(bytes::<32>(), [0; 32]);
    }

    #[test]
    fn gives_a_different_random_value_each_time() {
        assert_ne!(nonce(), nonce());
    }
}

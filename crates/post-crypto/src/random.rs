use post_proto::{Code, Nonce};

const CHARACTERS: u8 = 62;
const FIRST_UNFAIR_BYTE: u8 = u8::MAX - u8::MAX % CHARACTERS;

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

fn code_from(mut source: impl FnMut() -> [u8; 32]) -> Code {
    let mut text = String::with_capacity(Code::LENGTH);
    while text.len() < Code::LENGTH {
        for byte in source() {
            if byte < FIRST_UNFAIR_BYTE && text.len() < Code::LENGTH {
                let index = usize::from(byte % CHARACTERS);
                text.push(char::from(Code::ALPHABET[index]));
            }
        }
    }
    Code::parse(&text).expect("the alphabet has only letters and digits")
}

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

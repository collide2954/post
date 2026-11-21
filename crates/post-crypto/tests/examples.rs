use std::fs;
use std::path::{Path, PathBuf};

use post_crypto::{Exchange, Seed};
use post_proto::{ClientMessage, ErrorCode, Key, Nonce, ServerMessage};

enum Line {
    Client(ClientMessage),
    Server(ServerMessage),
}

struct Challenge {
    address: String,
    client: Nonce,
    server_key: Key,
    server: Nonce,
}

impl Challenge {
    fn exchange(&self) -> Exchange<'_> {
        Exchange {
            address: &self.address,
            client: &self.client,
            server: &self.server,
        }
    }
}

fn examples() -> Vec<PathBuf> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/protocol/examples");
    let mut paths: Vec<PathBuf> = fs::read_dir(directory)
        .expect("the directory of the examples")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    paths.sort();
    paths
}

fn lines(path: &Path) -> Vec<Line> {
    fs::read_to_string(path)
        .expect("an example")
        .lines()
        .filter_map(|line| {
            if let Some(json) = line.strip_prefix("C: ") {
                Some(Line::Client(ClientMessage::decode(json).expect(json)))
            } else {
                line.strip_prefix("S: ")
                    .map(|json| Line::Server(ServerMessage::decode(json).expect(json)))
            }
        })
        .collect()
}

#[test]
fn each_proof_in_the_example_conversations_is_correct() {
    let seed = Seed::from_bytes(std::array::from_fn(|index| {
        u8::try_from(index).expect("a small index")
    }));
    let mut server_proofs = 0;
    let mut account_proofs = 0;

    for path in examples() {
        let lines = lines(&path);
        let mut hello = None;
        let mut challenge = None;
        for (index, line) in lines.iter().enumerate() {
            let place = format!("{}, message {}", path.display(), index + 1);
            match line {
                Line::Client(ClientMessage::Hello { address, nonce, .. }) => {
                    hello = Some((address.clone(), *nonce));
                }
                Line::Server(ServerMessage::Challenge {
                    key, nonce, proof, ..
                }) => {
                    let (address, client) = hello.clone().expect("a hello before the challenge");
                    let answer = Challenge {
                        address,
                        client,
                        server_key: *key,
                        server: *nonce,
                    };
                    assert!(
                        answer.exchange().server_proof_is_valid(key, proof),
                        "{place}"
                    );
                    challenge = Some(answer);
                    server_proofs += 1;
                }
                Line::Client(
                    ClientMessage::Identify { key, proof, .. }
                    | ClientMessage::Register { key, proof, .. },
                ) => {
                    let Some(challenge) = &challenge else {
                        continue;
                    };
                    let is_refused = matches!(
                        lines.get(index + 1),
                        Some(Line::Server(ServerMessage::Error {
                            code: ErrorCode::InvalidProof,
                            ..
                        }))
                    );
                    let is_valid = challenge.exchange().account_proof_is_valid(
                        key,
                        &challenge.server_key,
                        proof,
                    );
                    assert_eq!(is_valid, !is_refused, "{place}");
                    assert_eq!(
                        *key,
                        seed.account_key(&challenge.server_key).public(),
                        "{place}"
                    );
                    account_proofs += 1;
                }
                _ => {}
            }
        }
    }

    assert!(server_proofs >= 4, "only {server_proofs} server proofs");
    assert!(account_proofs >= 8, "only {account_proofs} account proofs");
}

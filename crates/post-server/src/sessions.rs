use std::collections::HashMap;

use axum::extract::ws::Utf8Bytes;
use post_proto::{Code, ServerMessage, Timestamp};
use tokio::sync::mpsc::Sender;

use crate::hub::{ConnectionId, Hub, Outgoing};

pub type Line = Utf8Bytes;

const SHARED_LINES: usize = 4;

pub struct Sessions {
    hub: Hub,
    queues: HashMap<ConnectionId, Sender<Line>>,
    clock: fn() -> Timestamp,
}

impl Sessions {
    pub fn new(hub: Hub, clock: fn() -> Timestamp) -> Self {
        Self {
            hub,
            queues: HashMap::new(),
            clock,
        }
    }

    #[must_use]
    pub fn open_link(&self, code: &Code) -> Option<String> {
        self.hub.open_link(code, (self.clock)())
    }

    pub fn connect(&mut self, queue: Sender<Line>) -> ConnectionId {
        let id = self.hub.connect();
        self.queues.insert(id, queue);
        id
    }

    pub fn receive(&mut self, from: ConnectionId, line: &str) {
        let outgoing = self.hub.receive(from, line, (self.clock)());
        self.send(outgoing);
    }

    pub fn refuse(&mut self, from: ConnectionId) {
        let outgoing = self.hub.refuse(from);
        self.send(outgoing);
    }

    pub fn disconnect(&mut self, id: ConnectionId) {
        self.queues.remove(&id);
        let outgoing = self.hub.disconnect(id, (self.clock)());
        self.send(outgoing);
    }

    fn send(&mut self, outgoing: Vec<Outgoing>) {
        let mut shared: Vec<(ServerMessage, Line)> = Vec::new();
        for Outgoing { to, message } in outgoing {
            let Some(queue) = self.queues.get(&to) else {
                continue;
            };
            let known = shared.iter().find(|(known, _)| *known == message);
            let line = if let Some((_, line)) = known {
                line.clone()
            } else {
                let line = Line::from(message.encode());
                if shared.len() < SHARED_LINES {
                    shared.push((message, line.clone()));
                }
                line
            };
            if queue.try_send(line).is_err() {
                self.queues.remove(&to);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use post_crypto::{Exchange, Seed};
    use post_proto::{ClientMessage, ErrorCode, Name, Nonce, Role, ServerMessage, Timestamp};
    use tokio::sync::mpsc::{self, Receiver, error::TryRecvError};

    use super::{Line, Sessions};
    use crate::hub::{ConnectionId, Hub, LIMITS, Random, Settings};
    use crate::store::Store;

    const ADDRESS: &str = "localhost:7678";
    const PEOPLE: [&str; 2] = ["ana", "ben"];

    fn clock() -> Timestamp {
        Timestamp::parse("2026-11-21T14:02:07.120Z").expect("valid")
    }

    fn name(text: &str) -> Name {
        Name::parse(text).expect(text)
    }

    fn seed(who: &str) -> Seed {
        let person = PEOPLE.iter().position(|known| *known == who);
        let number = u8::try_from(person.expect("a known person")).expect("few people");
        Seed::from_bytes([number + 1; 32])
    }

    fn sessions() -> Sessions {
        let mut store = Store::in_memory().expect("a database in memory");
        let server = store.server_key().expect("a server key").public();
        let mut rules = store.rules(LIMITS).expect("the rules of an empty store");
        let link = post_crypto::code();
        rules
            .owner_code(&mut store, &link, clock())
            .expect("stored")
            .expect("the code is free");
        let key = |who: &str| seed(who).account_key(&server).public();
        let ana = rules
            .register(
                &mut store,
                key("ana").as_bytes(),
                &name("ana"),
                &link,
                clock(),
            )
            .expect("stored")
            .expect("the owner link is open");
        let invitation = post_crypto::code();
        rules
            .invite(&mut store, ana.id, &invitation, None, None, clock())
            .expect("stored")
            .expect("the owner can invite");
        rules
            .register(
                &mut store,
                key("ben").as_bytes(),
                &name("ben"),
                &invitation,
                clock(),
            )
            .expect("stored")
            .expect("the invitation is open");
        let settings = Settings {
            addresses: vec![ADDRESS.to_owned()],
            link: "http://localhost:7678".to_owned(),
            requires: Vec::new(),
        };
        let hub = Hub::new(store, settings, Random::default()).expect("a hub");
        Sessions::new(hub, clock)
    }

    fn send(text: &str) -> String {
        ClientMessage::Send {
            reference: None,
            channel: "lobby".to_owned(),
            text: text.to_owned(),
        }
        .encode()
    }

    fn join(who: &str) -> ServerMessage {
        ServerMessage::Join {
            channel: "lobby".to_owned(),
            name: name(who),
            time: clock(),
        }
    }

    fn message(from: &str, text: &str) -> ServerMessage {
        ServerMessage::Message {
            reference: None,
            channel: "lobby".to_owned(),
            from: name(from),
            time: clock(),
            text: text.to_owned(),
        }
    }

    fn user(sessions: &mut Sessions, who: &str, capacity: usize) -> (ConnectionId, Receiver<Line>) {
        let (queue, mut lines) = mpsc::channel(capacity);
        let id = sessions.connect(queue);
        let client = Nonce::from_bytes([200; 32]);
        let hello = ClientMessage::Hello {
            reference: None,
            protocol: 0,
            address: ADDRESS.to_owned(),
            nonce: client,
            supports: Vec::new(),
        };
        sessions.receive(id, &hello.encode());
        let challenge = lines
            .try_recv()
            .expect("the server answers the hello message");
        let Ok(ServerMessage::Challenge { key, nonce, .. }) = ServerMessage::decode(&challenge)
        else {
            panic!("the server sends a challenge, not {challenge}");
        };
        let exchange = Exchange {
            address: ADDRESS,
            client: &client,
            server: &nonce,
        };
        let account = seed(who).account_key(&key);
        let identify = ClientMessage::Identify {
            reference: None,
            key: account.public(),
            proof: account.prove(&key, &exchange),
            invitation: None,
        };
        sessions.receive(id, &identify.encode());
        (id, lines)
    }

    fn drain(lines: &mut Receiver<Line>) -> Vec<ServerMessage> {
        let mut messages = Vec::new();
        while let Ok(line) = lines.try_recv() {
            messages.push(ServerMessage::decode(&line).expect("a valid line"));
        }
        messages
    }

    #[test]
    fn puts_each_answer_into_the_queue_of_its_connection() {
        let mut sessions = sessions();
        let (ana, mut ana_lines) = user(&mut sessions, "ana", 8);
        let (_ben, mut ben_lines) = user(&mut sessions, "ben", 8);

        sessions.receive(ana, &send("Hello"));

        assert_eq!(
            drain(&mut ana_lines),
            [
                ServerMessage::Welcome {
                    reference: None,
                    name: name("ana"),
                    role: Role::Owner,
                },
                join("ana"),
                join("ben"),
                message("ana", "Hello"),
            ]
        );
        assert_eq!(
            drain(&mut ben_lines),
            [
                ServerMessage::Welcome {
                    reference: None,
                    name: name("ben"),
                    role: Role::Member,
                },
                join("ben"),
                message("ana", "Hello"),
            ]
        );
    }

    #[test]
    fn gives_one_copy_of_a_line_to_the_connections_that_get_the_same_message() {
        let mut sessions = sessions();
        let (ana, mut ana_lines) = user(&mut sessions, "ana", 8);
        let (_ben, mut ben_lines) = user(&mut sessions, "ben", 8);
        let (_ben_again, mut more_lines) = user(&mut sessions, "ben", 8);
        for lines in [&mut ana_lines, &mut ben_lines, &mut more_lines] {
            drain(lines);
        }
        let hello = ClientMessage::Send {
            reference: Some(7),
            channel: "lobby".to_owned(),
            text: "Hello".to_owned(),
        };

        sessions.receive(ana, &hello.encode());

        let to_ana = ana_lines.try_recv().expect("a line for the sender");
        let to_ben = ben_lines.try_recv().expect("a line for ben");
        let to_ben_again = more_lines.try_recv().expect("a line for ben");
        assert_eq!(to_ben.as_str(), to_ben_again.as_str());
        assert_eq!(to_ben.as_ptr(), to_ben_again.as_ptr());
        assert_ne!(to_ana.as_str(), to_ben.as_str());
        assert!(to_ana.as_str().contains("\"ref\":7"), "{to_ana}");
    }

    #[test]
    fn answers_data_that_is_not_text_with_an_error() {
        let mut sessions = sessions();
        let (queue, mut lines) = mpsc::channel(8);
        let id = sessions.connect(queue);

        sessions.refuse(id);

        assert_eq!(
            drain(&mut lines),
            [ServerMessage::error(None, ErrorCode::BadMessage)]
        );
    }

    #[test]
    fn closes_the_queue_of_a_connection_that_does_not_read() {
        let mut sessions = sessions();
        let (ana, mut ana_lines) = user(&mut sessions, "ana", 16);
        let (_ben, mut ben_lines) = user(&mut sessions, "ben", 4);

        for text in ["one", "two", "three", "four"] {
            sessions.receive(ana, &send(text));
        }

        let ben_messages = drain(&mut ben_lines);
        assert_eq!(ben_messages.len(), 4);
        assert_eq!(ben_messages[3], message("ana", "two"));
        assert_eq!(ben_lines.try_recv(), Err(TryRecvError::Disconnected));

        assert_eq!(drain(&mut ana_lines).len(), 7);
        assert_eq!(ana_lines.try_recv(), Err(TryRecvError::Empty));
    }

    #[test]
    fn tells_the_other_users_when_a_connection_closes() {
        let mut sessions = sessions();
        let (_ana, mut ana_lines) = user(&mut sessions, "ana", 8);
        let (ben, mut ben_lines) = user(&mut sessions, "ben", 8);
        drain(&mut ana_lines);
        drain(&mut ben_lines);

        sessions.disconnect(ben);

        assert_eq!(
            drain(&mut ana_lines),
            [ServerMessage::Leave {
                channel: "lobby".to_owned(),
                name: name("ben"),
                time: clock(),
            }]
        );
        assert_eq!(ben_lines.try_recv(), Err(TryRecvError::Disconnected));
    }
}

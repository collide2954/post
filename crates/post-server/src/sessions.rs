use std::collections::HashMap;

use post_proto::Timestamp;
use tokio::sync::mpsc::Sender;

use crate::hub::{ConnectionId, Hub, Outgoing};

pub struct Sessions {
    hub: Hub,
    queues: HashMap<ConnectionId, Sender<String>>,
    clock: fn() -> Timestamp,
}

impl Sessions {
    pub fn new(clock: fn() -> Timestamp) -> Self {
        Self {
            hub: Hub::new(),
            queues: HashMap::new(),
            clock,
        }
    }

    pub fn connect(&mut self, queue: Sender<String>) -> ConnectionId {
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
        for Outgoing { to, message } in outgoing {
            let lost = self
                .queues
                .get(&to)
                .is_some_and(|queue| queue.try_send(message.encode()).is_err());
            if lost {
                self.queues.remove(&to);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use post_proto::{ClientMessage, ErrorCode, Name, ServerMessage, Timestamp};
    use tokio::sync::mpsc::{self, Receiver, error::TryRecvError};

    use super::Sessions;
    use crate::hub::ConnectionId;

    fn clock() -> Timestamp {
        Timestamp::parse("2026-11-21T14:02:07.120Z").expect("valid")
    }

    fn name(text: &str) -> Name {
        Name::parse(text).expect(text)
    }

    fn hello(name: &str) -> String {
        ClientMessage::Hello {
            reference: None,
            protocol: 0,
            name: name.to_owned(),
        }
        .encode()
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

    fn user(
        sessions: &mut Sessions,
        who: &str,
        capacity: usize,
    ) -> (ConnectionId, Receiver<String>) {
        let (queue, lines) = mpsc::channel(capacity);
        let id = sessions.connect(queue);
        sessions.receive(id, &hello(who));
        (id, lines)
    }

    fn drain(lines: &mut Receiver<String>) -> Vec<ServerMessage> {
        let mut messages = Vec::new();
        while let Ok(line) = lines.try_recv() {
            messages.push(ServerMessage::decode(&line).expect("a valid line"));
        }
        messages
    }

    #[test]
    fn puts_each_answer_into_the_queue_of_its_connection() {
        let mut sessions = Sessions::new(clock);
        let (ana, mut ana_lines) = user(&mut sessions, "ana", 8);
        let (_ben, mut ben_lines) = user(&mut sessions, "ben", 8);

        sessions.receive(ana, &send("Hello"));

        assert_eq!(
            drain(&mut ana_lines),
            [
                ServerMessage::Welcome {
                    reference: None,
                    protocol: 0,
                    name: name("ana"),
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
                    protocol: 0,
                    name: name("ben"),
                },
                join("ben"),
                message("ana", "Hello"),
            ]
        );
    }

    #[test]
    fn answers_data_that_is_not_text_with_an_error() {
        let mut sessions = Sessions::new(clock);
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
        let mut sessions = Sessions::new(clock);
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
        let mut sessions = Sessions::new(clock);
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

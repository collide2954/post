use std::collections::BTreeMap;

use post_proto::{
    ClientMessage, ErrorCode, Name, PROTOCOL_VERSION, ServerMessage, TextError, Timestamp,
    check_text,
};

pub const LOBBY: &str = "lobby";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectionId(u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    pub to: ConnectionId,
    pub message: ServerMessage,
}

#[derive(Debug, Default)]
pub struct Hub {
    next_id: u64,
    connections: BTreeMap<ConnectionId, Option<Name>>,
}

impl Hub {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn connect(&mut self) -> ConnectionId {
        let id = ConnectionId(self.next_id);
        self.next_id += 1;
        self.connections.insert(id, None);
        id
    }

    pub fn receive(&mut self, from: ConnectionId, line: &str, now: Timestamp) -> Vec<Outgoing> {
        let Some(name) = self.connections.get(&from).cloned() else {
            return Vec::new();
        };
        match ClientMessage::decode(line) {
            Err(_) => refusal(from, None, ErrorCode::BadMessage),
            Ok(ClientMessage::Hello {
                reference,
                protocol,
                name: wanted,
            }) => match self.admit(name.is_some(), protocol, &wanted) {
                Ok(name) => self.welcome(from, reference, &name, now),
                Err(code) => refusal(from, reference, code),
            },
            Ok(ClientMessage::Send {
                reference,
                channel,
                text,
            }) => match examine_text(name, &channel, &text) {
                Ok(name) => self.deliver(from, reference, &name, &text, now),
                Err(code) => refusal(from, reference, code),
            },
        }
    }

    #[must_use]
    pub fn refuse(&self, from: ConnectionId) -> Vec<Outgoing> {
        if self.connections.contains_key(&from) {
            refusal(from, None, ErrorCode::BadMessage)
        } else {
            Vec::new()
        }
    }

    pub fn disconnect(&mut self, id: ConnectionId, now: Timestamp) -> Vec<Outgoing> {
        let Some(Some(name)) = self.connections.remove(&id) else {
            return Vec::new();
        };
        self.to_each_member(|_| ServerMessage::Leave {
            channel: LOBBY.to_owned(),
            name: name.clone(),
            time: now,
        })
    }

    fn admit(&self, has_name: bool, protocol: u32, wanted: &str) -> Result<Name, ErrorCode> {
        if has_name {
            return Err(ErrorCode::DuplicateHello);
        }
        if protocol != PROTOCOL_VERSION {
            return Err(ErrorCode::UnsupportedProtocol);
        }
        let name = Name::parse(wanted).map_err(|_| ErrorCode::InvalidName)?;
        if self.members().any(|(_, member)| *member == name) {
            return Err(ErrorCode::NameTaken);
        }
        Ok(name)
    }

    fn welcome(
        &mut self,
        id: ConnectionId,
        reference: Option<u32>,
        name: &Name,
        now: Timestamp,
    ) -> Vec<Outgoing> {
        self.connections.insert(id, Some(name.clone()));
        let mut outgoing = vec![Outgoing {
            to: id,
            message: ServerMessage::Welcome {
                reference,
                protocol: PROTOCOL_VERSION,
                name: name.clone(),
            },
        }];
        outgoing.extend(self.to_each_member(|_| ServerMessage::Join {
            channel: LOBBY.to_owned(),
            name: name.clone(),
            time: now,
        }));
        outgoing
    }

    fn deliver(
        &self,
        from: ConnectionId,
        reference: Option<u32>,
        name: &Name,
        text: &str,
        now: Timestamp,
    ) -> Vec<Outgoing> {
        self.to_each_member(|to| ServerMessage::Message {
            reference: reference.filter(|_| to == from),
            channel: LOBBY.to_owned(),
            from: name.clone(),
            time: now,
            text: text.to_owned(),
        })
    }

    fn members(&self) -> impl Iterator<Item = (ConnectionId, &Name)> {
        self.connections
            .iter()
            .filter_map(|(id, name)| name.as_ref().map(|name| (*id, name)))
    }

    fn to_each_member(&self, message: impl Fn(ConnectionId) -> ServerMessage) -> Vec<Outgoing> {
        self.members()
            .map(|(to, _)| Outgoing {
                to,
                message: message(to),
            })
            .collect()
    }
}

fn examine_text(name: Option<Name>, channel: &str, text: &str) -> Result<Name, ErrorCode> {
    let name = name.ok_or(ErrorCode::HelloRequired)?;
    if channel != LOBBY {
        return Err(ErrorCode::NoSuchChannel);
    }
    check_text(text).map_err(|error| match error {
        TextError::Empty => ErrorCode::EmptyText,
        TextError::TooLong => ErrorCode::TextTooLong,
    })?;
    Ok(name)
}

fn refusal(to: ConnectionId, reference: Option<u32>, code: ErrorCode) -> Vec<Outgoing> {
    vec![Outgoing {
        to,
        message: ServerMessage::error(reference, code),
    }]
}

#[cfg(test)]
mod tests {
    use post_proto::{ClientMessage, ErrorCode, Name, ServerMessage, Timestamp};

    use super::{ConnectionId, Hub, LOBBY, Outgoing};

    fn now() -> Timestamp {
        Timestamp::parse("2026-11-21T14:02:07.120Z").expect("valid")
    }

    fn later() -> Timestamp {
        Timestamp::parse("2026-11-21T14:03:00.000Z").expect("valid")
    }

    fn name(text: &str) -> Name {
        Name::parse(text).expect(text)
    }

    fn hello(reference: u32, name: &str) -> String {
        ClientMessage::Hello {
            reference: Some(reference),
            protocol: 0,
            name: name.to_owned(),
        }
        .encode()
    }

    fn send(reference: Option<u32>, channel: &str, text: &str) -> String {
        ClientMessage::Send {
            reference,
            channel: channel.to_owned(),
            text: text.to_owned(),
        }
        .encode()
    }

    fn to(to: ConnectionId, message: ServerMessage) -> Outgoing {
        Outgoing { to, message }
    }

    fn join(who: &str) -> ServerMessage {
        ServerMessage::Join {
            channel: LOBBY.to_owned(),
            name: name(who),
            time: now(),
        }
    }

    fn error(reference: Option<u32>, code: ErrorCode) -> ServerMessage {
        ServerMessage::error(reference, code)
    }

    fn hub_with(names: &[&str]) -> (Hub, Vec<ConnectionId>) {
        let mut hub = Hub::new();
        let ids = names
            .iter()
            .map(|name| {
                let id = hub.connect();
                hub.receive(id, &hello(0, name), now());
                id
            })
            .collect();
        (hub, ids)
    }

    #[test]
    fn the_channel_has_the_name_lobby() {
        assert_eq!(LOBBY, "lobby");
    }

    #[test]
    fn gives_each_connection_a_different_identifier() {
        let mut hub = Hub::new();
        let first = hub.connect();
        let second = hub.connect();
        assert_ne!(first, second);
    }

    #[test]
    fn welcomes_a_user_and_puts_the_user_into_the_lobby() {
        let mut hub = Hub::new();
        let ana = hub.connect();

        let outgoing = hub.receive(ana, &hello(1, "ana"), now());

        assert_eq!(
            outgoing,
            [
                to(
                    ana,
                    ServerMessage::Welcome {
                        reference: Some(1),
                        protocol: 0,
                        name: name("ana"),
                    }
                ),
                to(ana, join("ana")),
            ]
        );
    }

    #[test]
    fn tells_each_user_in_the_lobby_about_a_new_user() {
        let (mut hub, ids) = hub_with(&["ana"]);
        let ben = hub.connect();

        let outgoing = hub.receive(ben, &hello(9, "ben"), now());

        assert_eq!(
            outgoing,
            [
                to(
                    ben,
                    ServerMessage::Welcome {
                        reference: Some(9),
                        protocol: 0,
                        name: name("ben"),
                    }
                ),
                to(ids[0], join("ben")),
                to(ben, join("ben")),
            ]
        );
    }

    #[test]
    fn sends_nothing_to_a_connection_without_a_name() {
        let (mut hub, ids) = hub_with(&["ana"]);
        let silent = hub.connect();
        let ben = hub.connect();

        let joined = hub.receive(ben, &hello(1, "ben"), now());
        let sent = hub.receive(ids[0], &send(None, "lobby", "Hello"), now());
        let left = hub.disconnect(ben, later());

        for outgoing in joined.iter().chain(&sent).chain(&left) {
            assert_ne!(outgoing.to, silent);
        }
    }

    #[test]
    fn sends_a_text_to_each_user_and_gives_the_number_only_to_the_sender() {
        let (mut hub, ids) = hub_with(&["ana", "ben", "cy"]);

        let outgoing = hub.receive(ids[1], &send(Some(4), "lobby", "Hello"), now());

        let message = |reference| ServerMessage::Message {
            reference,
            channel: "lobby".to_owned(),
            from: name("ben"),
            time: now(),
            text: "Hello".to_owned(),
        };
        assert_eq!(
            outgoing,
            [
                to(ids[0], message(None)),
                to(ids[1], message(Some(4))),
                to(ids[2], message(None)),
            ]
        );
    }

    #[test]
    fn keeps_the_text_as_the_user_wrote_it() {
        let (mut hub, ids) = hub_with(&["ana"]);

        let outgoing = hub.receive(ids[0], &send(None, "lobby", "  two\n  lines  "), now());

        assert_eq!(
            outgoing,
            [to(
                ids[0],
                ServerMessage::Message {
                    reference: None,
                    channel: "lobby".to_owned(),
                    from: name("ana"),
                    time: now(),
                    text: "  two\n  lines  ".to_owned(),
                }
            )]
        );
    }

    #[test]
    fn refuses_a_line_that_is_not_a_message() {
        let (mut hub, ids) = hub_with(&["ana"]);

        for line in ["", "hello", r#"{"type":"dance","ref":3}"#] {
            assert_eq!(
                hub.receive(ids[0], line, now()),
                [to(ids[0], error(None, ErrorCode::BadMessage))],
                "{line}"
            );
        }
    }

    #[test]
    fn refuses_data_that_is_not_text() {
        let mut hub = Hub::new();
        let ana = hub.connect();

        assert_eq!(
            hub.refuse(ana),
            [to(ana, error(None, ErrorCode::BadMessage))]
        );
    }

    #[test]
    fn refuses_a_text_before_the_hello_message() {
        let mut hub = Hub::new();
        let ana = hub.connect();

        assert_eq!(
            hub.receive(ana, &send(Some(2), "nowhere", ""), now()),
            [to(ana, error(Some(2), ErrorCode::HelloRequired))]
        );
    }

    #[test]
    fn refuses_a_hello_message_after_the_welcome() {
        let (mut hub, ids) = hub_with(&["ana"]);

        let invalid = ClientMessage::Hello {
            reference: Some(6),
            protocol: 99,
            name: "Not A Name".to_owned(),
        }
        .encode();

        assert_eq!(
            hub.receive(ids[0], &invalid, now()),
            [to(ids[0], error(Some(6), ErrorCode::DuplicateHello))]
        );
        assert_eq!(
            hub.receive(ids[0], &hello(7, "ana2"), now()),
            [to(ids[0], error(Some(7), ErrorCode::DuplicateHello))]
        );
    }

    #[test]
    fn refuses_a_protocol_version_that_it_does_not_support() {
        let mut hub = Hub::new();
        let ana = hub.connect();
        let line = ClientMessage::Hello {
            reference: Some(1),
            protocol: 1,
            name: "Not A Name".to_owned(),
        }
        .encode();

        assert_eq!(
            hub.receive(ana, &line, now()),
            [to(ana, error(Some(1), ErrorCode::UnsupportedProtocol))]
        );
    }

    #[test]
    fn refuses_a_name_that_breaks_the_rules() {
        let (mut hub, _) = hub_with(&["ana"]);
        let new = hub.connect();

        for invalid in ["Ana", "a", "a name", "abcdefghij0123456789x"] {
            assert_eq!(
                hub.receive(new, &hello(3, invalid), now()),
                [to(new, error(Some(3), ErrorCode::InvalidName))],
                "{invalid}"
            );
        }
    }

    #[test]
    fn refuses_a_name_that_a_connected_user_has() {
        let (mut hub, _) = hub_with(&["ana", "ben"]);
        let new = hub.connect();

        assert_eq!(
            hub.receive(new, &hello(3, "ben"), now()),
            [to(new, error(Some(3), ErrorCode::NameTaken))]
        );
    }

    #[test]
    fn accepts_a_new_hello_message_after_an_error() {
        let (mut hub, ids) = hub_with(&["ana"]);
        let new = hub.connect();
        hub.receive(new, &hello(1, "ana"), now());

        let outgoing = hub.receive(new, &hello(2, "ben"), now());

        assert_eq!(
            outgoing,
            [
                to(
                    new,
                    ServerMessage::Welcome {
                        reference: Some(2),
                        protocol: 0,
                        name: name("ben"),
                    }
                ),
                to(ids[0], join("ben")),
                to(new, join("ben")),
            ]
        );
    }

    #[test]
    fn refuses_a_channel_that_does_not_exist() {
        let (mut hub, ids) = hub_with(&["ana"]);

        for channel in ["garden", "#lobby", "Lobby", ""] {
            assert_eq!(
                hub.receive(ids[0], &send(Some(8), channel, ""), now()),
                [to(ids[0], error(Some(8), ErrorCode::NoSuchChannel))],
                "{channel}"
            );
        }
    }

    #[test]
    fn refuses_a_text_that_breaks_the_rules() {
        let (mut hub, ids) = hub_with(&["ana", "ben"]);

        assert_eq!(
            hub.receive(ids[0], &send(Some(1), "lobby", " \n "), now()),
            [to(ids[0], error(Some(1), ErrorCode::EmptyText))]
        );
        assert_eq!(
            hub.receive(ids[0], &send(None, "lobby", &"x".repeat(4001)), now()),
            [to(ids[0], error(None, ErrorCode::TextTooLong))]
        );
    }

    #[test]
    fn tells_the_other_users_when_a_user_leaves() {
        let (mut hub, ids) = hub_with(&["ana", "ben", "cy"]);

        let outgoing = hub.disconnect(ids[1], later());

        let leave = ServerMessage::Leave {
            channel: "lobby".to_owned(),
            name: name("ben"),
            time: later(),
        };
        assert_eq!(outgoing, [to(ids[0], leave.clone()), to(ids[2], leave)]);
    }

    #[test]
    fn makes_a_name_free_when_its_user_leaves() {
        let (mut hub, ids) = hub_with(&["ana"]);
        hub.disconnect(ids[0], later());
        let new = hub.connect();

        let outgoing = hub.receive(new, &hello(1, "ana"), now());

        assert_eq!(outgoing.len(), 2);
        assert_eq!(outgoing[1], to(new, join("ana")));
    }

    #[test]
    fn is_silent_when_a_connection_without_a_name_closes() {
        let (mut hub, _) = hub_with(&["ana"]);
        let silent = hub.connect();

        assert_eq!(hub.disconnect(silent, later()), []);
    }

    #[test]
    fn ignores_a_connection_that_it_does_not_know() {
        let (mut hub, ids) = hub_with(&["ana"]);
        hub.disconnect(ids[0], later());

        assert_eq!(hub.receive(ids[0], &hello(1, "ana"), now()), []);
        assert_eq!(hub.refuse(ids[0]), []);
        assert_eq!(hub.disconnect(ids[0], later()), []);
    }
}

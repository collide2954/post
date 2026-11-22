use std::collections::BTreeMap;

use post_crypto::{Exchange, ServerKey};
use post_proto::{
    ClientMessage, Code, ErrorCode, Key, LOBBY, Name, Nonce, PROTOCOL_VERSION, Part, Role,
    ServerMessage, Signature, TextError, Timestamp, check_text,
};

use post_rules::{Account, AccountId, Limits, RegisterError, Rules};

use crate::store::{Store, StoreError, Stored};

const DAY: u64 = 24 * 60 * 60 * 1000;
pub const LIMITS: Limits = Limits {
    rename_interval: 30 * DAY,
    name_reserved: 30 * DAY,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectionId(u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    pub to: ConnectionId,
    pub message: ServerMessage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub addresses: Vec<String>,
    pub link: String,
    pub requires: Vec<Part>,
}

pub struct Random {
    pub nonce: Box<dyn FnMut() -> Nonce + Send>,
    pub code: Box<dyn FnMut() -> Code + Send>,
}

pub struct Hub {
    store: Store,
    rules: Rules,
    key: ServerKey,
    settings: Settings,
    random: Random,
    next_id: u64,
    connections: BTreeMap<ConnectionId, State>,
}

#[derive(Debug, Clone)]
enum State {
    New,
    Challenged(Challenge),
    Present(Member),
}

#[derive(Debug, Clone)]
struct Challenge {
    address: String,
    client: Nonce,
    server: Nonce,
    supports: Vec<Part>,
}

#[derive(Debug, Clone)]
struct Member {
    account: Option<AccountId>,
    name: Name,
    role: Role,
}

struct Credentials {
    key: Key,
    proof: Signature,
}

type Refused = ErrorCode;

impl Default for Random {
    fn default() -> Self {
        Self {
            nonce: Box::new(post_crypto::nonce),
            code: Box::new(post_crypto::code),
        }
    }
}

impl Hub {
    pub fn new(mut store: Store, settings: Settings, random: Random) -> Stored<Self> {
        let key = store.server_key()?;
        let rules = store.rules(LIMITS)?;
        Ok(Self {
            store,
            rules,
            key,
            settings,
            random,
            next_id: 0,
            connections: BTreeMap::new(),
        })
    }

    pub fn owner_link(&mut self, now: Timestamp) -> Stored<Option<String>> {
        let fresh = (self.random.code)();
        let code = self
            .rules
            .owner_code(&mut self.store, &fresh, now)?
            .map_err(|_| StoreError::CodeInUse)?;
        Ok(code.map(|code| format!("{}/i/{code}", self.settings.link)))
    }

    pub fn connect(&mut self) -> ConnectionId {
        let id = ConnectionId(self.next_id);
        self.next_id += 1;
        self.connections.insert(id, State::New);
        id
    }

    pub fn receive(&mut self, from: ConnectionId, line: &str, now: Timestamp) -> Vec<Outgoing> {
        let Some(state) = self.connections.get(&from).cloned() else {
            return Vec::new();
        };
        let Ok(message) = ClientMessage::decode(line) else {
            return refusal(from, None, ErrorCode::BadMessage);
        };
        let (reference, result) = match message {
            ClientMessage::Hello {
                reference,
                protocol,
                address,
                nonce,
                supports,
            } => (
                reference,
                self.challenge(from, reference, &state, protocol, address, nonce, supports),
            ),
            ClientMessage::Identify {
                reference,
                key,
                proof,
                invitation,
            } => (
                reference,
                self.identify(
                    &state,
                    &Credentials { key, proof },
                    invitation.as_ref(),
                    now,
                )
                .map(|member| self.enter(from, reference, member, now)),
            ),
            ClientMessage::Register {
                reference,
                key,
                proof,
                name,
                invitation,
            } => (
                reference,
                self.register(&state, &Credentials { key, proof }, &name, &invitation, now)
                    .map(|member| self.enter(from, reference, member, now)),
            ),
            ClientMessage::Guest {
                reference,
                protocol,
                name,
            } => (
                reference,
                self.admit_guest(&state, protocol, &name)
                    .map(|member| self.enter(from, reference, member, now)),
            ),
            ClientMessage::Send {
                reference,
                channel,
                text,
            } => (
                reference,
                examine_text(&state, &channel, &text)
                    .map(|member| self.deliver(from, reference, &member.name, &text, now)),
            ),
        };
        result.unwrap_or_else(|code| refusal(from, reference, code))
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
        let Some(State::Present(member)) = self.connections.remove(&id) else {
            return Vec::new();
        };
        if self.has_a_connection(&member) {
            return Vec::new();
        }
        self.to_each_member(|_| ServerMessage::Leave {
            channel: LOBBY.to_owned(),
            name: member.name.clone(),
            time: now,
        })
    }

    #[expect(clippy::too_many_arguments, reason = "the fields of one message")]
    fn challenge(
        &mut self,
        from: ConnectionId,
        reference: Option<u32>,
        state: &State,
        protocol: u32,
        address: String,
        client: Nonce,
        supports: Vec<Part>,
    ) -> Result<Vec<Outgoing>, Refused> {
        if !matches!(state, State::New) {
            return Err(ErrorCode::DuplicateHello);
        }
        if protocol != PROTOCOL_VERSION {
            return Err(ErrorCode::UnsupportedProtocol);
        }
        if !self.settings.addresses.contains(&address) {
            return Err(ErrorCode::WrongAddress);
        }
        let challenge = Challenge {
            address,
            client,
            server: (self.random.nonce)(),
            supports,
        };
        let message = ServerMessage::Challenge {
            reference,
            protocol: PROTOCOL_VERSION,
            key: self.key.public(),
            nonce: challenge.server,
            proof: self.key.prove(&challenge.exchange()),
            requires: self.settings.requires.clone(),
        };
        self.connections.insert(from, State::Challenged(challenge));
        Ok(vec![Outgoing { to: from, message }])
    }

    fn identify(
        &mut self,
        state: &State,
        credentials: &Credentials,
        invitation: Option<&Code>,
        now: Timestamp,
    ) -> Result<Member, Refused> {
        self.examine(state, credentials)?;
        let account = self
            .rules
            .account(credentials.key.as_bytes())
            .ok_or(ErrorCode::UnknownAccount)?;
        let id = account.id;
        let mut member = Member::of(account);
        self.examine_parts(state)?;
        if let Some(code) = invitation {
            let is_new_owner = self
                .rules
                .claim(&mut self.store, id, code, now)
                .map_err(|_| ErrorCode::ServerError)?;
            if is_new_owner {
                member.role = Role::Owner;
            }
        }
        Ok(member)
    }

    fn register(
        &mut self,
        state: &State,
        credentials: &Credentials,
        name: &str,
        invitation: &Code,
        now: Timestamp,
    ) -> Result<Member, Refused> {
        self.examine(state, credentials)?;
        self.examine_parts(state)?;
        let name = Name::parse(name).map_err(|_| ErrorCode::InvalidName)?;
        self.rules
            .register(
                &mut self.store,
                credentials.key.as_bytes(),
                &name,
                invitation,
                now,
            )
            .map_err(|_| ErrorCode::ServerError)?
            .map(|account| Member::of(&account))
            .map_err(|error| match error {
                RegisterError::AccountExists => ErrorCode::AccountExists,
                RegisterError::InvalidInvitation => ErrorCode::InvalidInvitation,
                RegisterError::NameTaken => ErrorCode::NameTaken,
                RegisterError::Full => ErrorCode::ServerError,
            })
    }

    fn examine(&self, state: &State, credentials: &Credentials) -> Result<(), Refused> {
        let challenge = match state {
            State::New => return Err(ErrorCode::HelloRequired),
            State::Present(_) => return Err(ErrorCode::DuplicateIdentify),
            State::Challenged(challenge) => challenge,
        };
        let is_valid = challenge.exchange().account_proof_is_valid(
            &credentials.key,
            &self.key.public(),
            &credentials.proof,
        );
        if is_valid {
            Ok(())
        } else {
            Err(ErrorCode::InvalidProof)
        }
    }

    fn examine_parts(&self, state: &State) -> Result<(), Refused> {
        let supports: &[Part] = match state {
            State::Challenged(challenge) => &challenge.supports,
            _ => &[],
        };
        if self
            .settings
            .requires
            .iter()
            .all(|part| supports.contains(part))
        {
            Ok(())
        } else {
            Err(ErrorCode::MissingPart)
        }
    }

    fn admit_guest(&self, state: &State, protocol: u32, wanted: &str) -> Result<Member, Refused> {
        if !matches!(state, State::New) {
            return Err(ErrorCode::DuplicateHello);
        }
        if protocol != PROTOCOL_VERSION {
            return Err(ErrorCode::UnsupportedProtocol);
        }
        let name = Name::parse(wanted).map_err(|_| ErrorCode::InvalidName)?;
        if self.members().any(|(_, member)| member.name == name) {
            return Err(ErrorCode::NameTaken);
        }
        Ok(Member {
            account: None,
            name,
            role: Role::Member,
        })
    }

    fn enter(
        &mut self,
        id: ConnectionId,
        reference: Option<u32>,
        member: Member,
        now: Timestamp,
    ) -> Vec<Outgoing> {
        let is_first_connection = !self.has_a_connection(&member);
        let mut outgoing = vec![Outgoing {
            to: id,
            message: ServerMessage::Welcome {
                reference,
                name: member.name.clone(),
                role: member.role,
            },
        }];
        let name = member.name.clone();
        self.connections.insert(id, State::Present(member));
        if is_first_connection {
            outgoing.extend(self.to_each_member(|_| ServerMessage::Join {
                channel: LOBBY.to_owned(),
                name: name.clone(),
                time: now,
            }));
        }
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

    fn has_a_connection(&self, member: &Member) -> bool {
        member.account.is_some()
            && self
                .members()
                .any(|(_, other)| other.account == member.account)
    }

    fn members(&self) -> impl Iterator<Item = (ConnectionId, &Member)> {
        self.connections
            .iter()
            .filter_map(|(id, state)| match state {
                State::Present(member) => Some((*id, member)),
                _ => None,
            })
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

impl Challenge {
    fn exchange(&self) -> Exchange<'_> {
        Exchange {
            address: &self.address,
            client: &self.client,
            server: &self.server,
        }
    }
}

impl Member {
    fn of(account: &Account) -> Self {
        Self {
            account: Some(account.id),
            name: account.name.clone(),
            role: if account.owner {
                Role::Owner
            } else {
                Role::Member
            },
        }
    }
}

fn examine_text<'a>(state: &'a State, channel: &str, text: &str) -> Result<&'a Member, Refused> {
    let member = match state {
        State::New => return Err(ErrorCode::HelloRequired),
        State::Challenged(_) => return Err(ErrorCode::IdentifyRequired),
        State::Present(member) => member,
    };
    if channel != LOBBY {
        return Err(ErrorCode::NoSuchChannel);
    }
    check_text(text).map_err(|error| match error {
        TextError::Empty => ErrorCode::EmptyText,
        TextError::TooLong => ErrorCode::TextTooLong,
    })?;
    Ok(member)
}

fn refusal(to: ConnectionId, reference: Option<u32>, code: ErrorCode) -> Vec<Outgoing> {
    vec![Outgoing {
        to,
        message: ServerMessage::error(reference, code),
    }]
}

#[cfg(test)]
mod tests {
    use post_crypto::{Exchange, Seed};
    use post_proto::{
        ClientMessage, Code, ErrorCode, Key, LOBBY, Name, Nonce, Part, Role, ServerMessage,
        Signature, Timestamp,
    };

    use post_rules::{Change, Invitation, Storage as _};

    use super::{ConnectionId, Hub, Outgoing, Random, Settings};
    use crate::store::{Store, StoreError};

    fn now() -> Timestamp {
        Timestamp::parse("2026-11-21T14:02:07.120Z").expect("valid")
    }

    fn later() -> Timestamp {
        Timestamp::parse("2026-11-21T14:03:00.000Z").expect("valid")
    }

    fn name(text: &str) -> Name {
        Name::parse(text).expect(text)
    }

    fn guest(reference: u32, name: &str) -> String {
        ClientMessage::Guest {
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

    fn settings() -> Settings {
        Settings {
            addresses: vec!["chat.example.org:443".to_owned()],
            link: "https://chat.example.org".to_owned(),
            requires: Vec::new(),
        }
    }

    fn new_hub() -> Hub {
        let store = Store::in_memory().expect("a database in memory");
        Hub::new(store, settings(), random()).expect("a hub")
    }

    fn hub_with(names: &[&str]) -> (Hub, Vec<ConnectionId>) {
        let mut hub = new_hub();
        let ids = names
            .iter()
            .map(|name| {
                let id = hub.connect();
                hub.receive(id, &guest(0, name), now());
                id
            })
            .collect();
        (hub, ids)
    }

    #[test]
    fn gives_each_connection_a_different_identifier() {
        let mut hub = new_hub();
        let first = hub.connect();
        let second = hub.connect();
        assert_ne!(first, second);
    }

    #[test]
    fn welcomes_a_user_and_puts_the_user_into_the_lobby() {
        let mut hub = new_hub();
        let ana = hub.connect();

        let outgoing = hub.receive(ana, &guest(1, "ana"), now());

        assert_eq!(
            outgoing,
            [
                to(
                    ana,
                    ServerMessage::Welcome {
                        reference: Some(1),
                        name: name("ana"),
                        role: Role::Member,
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

        let outgoing = hub.receive(ben, &guest(9, "ben"), now());

        assert_eq!(
            outgoing,
            [
                to(
                    ben,
                    ServerMessage::Welcome {
                        reference: Some(9),
                        name: name("ben"),
                        role: Role::Member,
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

        let joined = hub.receive(ben, &guest(1, "ben"), now());
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
        let mut hub = new_hub();
        let ana = hub.connect();

        assert_eq!(
            hub.refuse(ana),
            [to(ana, error(None, ErrorCode::BadMessage))]
        );
    }

    #[test]
    fn refuses_a_text_before_the_hello_message() {
        let mut hub = new_hub();
        let ana = hub.connect();

        assert_eq!(
            hub.receive(ana, &send(Some(2), "nowhere", ""), now()),
            [to(ana, error(Some(2), ErrorCode::HelloRequired))]
        );
    }

    #[test]
    fn refuses_a_hello_message_after_the_welcome() {
        let (mut hub, ids) = hub_with(&["ana"]);

        let invalid = ClientMessage::Guest {
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
            hub.receive(ids[0], &guest(7, "ana2"), now()),
            [to(ids[0], error(Some(7), ErrorCode::DuplicateHello))]
        );
    }

    #[test]
    fn refuses_a_protocol_version_that_it_does_not_support() {
        let mut hub = new_hub();
        let ana = hub.connect();
        let line = ClientMessage::Guest {
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
                hub.receive(new, &guest(3, invalid), now()),
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
            hub.receive(new, &guest(3, "ben"), now()),
            [to(new, error(Some(3), ErrorCode::NameTaken))]
        );
    }

    #[test]
    fn accepts_a_new_hello_message_after_an_error() {
        let (mut hub, ids) = hub_with(&["ana"]);
        let new = hub.connect();
        hub.receive(new, &guest(1, "ana"), now());

        let outgoing = hub.receive(new, &guest(2, "ben"), now());

        assert_eq!(
            outgoing,
            [
                to(
                    new,
                    ServerMessage::Welcome {
                        reference: Some(2),
                        name: name("ben"),
                        role: Role::Member,
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

        let outgoing = hub.receive(new, &guest(1, "ana"), now());

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

        assert_eq!(hub.receive(ids[0], &guest(1, "ana"), now()), []);
        assert_eq!(hub.refuse(ids[0]), []);
        assert_eq!(hub.disconnect(ids[0], later()), []);
    }

    const ADDRESS: &str = "chat.example.org:443";

    fn random() -> Random {
        let mut nonces = 0_u8;
        let mut codes = 0_u32;
        Random {
            nonce: Box::new(move || {
                nonces += 1;
                Nonce::from_bytes([nonces; 32])
            }),
            code: Box::new(move || {
                codes += 1;
                code(codes)
            }),
        }
    }

    fn code(number: u32) -> Code {
        Code::parse(&format!("code{number:08}")).expect("a valid code")
    }

    fn seed(person: u8) -> Seed {
        Seed::from_bytes([person; 32])
    }

    fn client_nonce() -> Nonce {
        Nonce::from_bytes([200; 32])
    }

    fn hello(reference: u32, address: &str, supports: &[&str]) -> String {
        ClientMessage::Hello {
            reference: Some(reference),
            protocol: 0,
            address: address.to_owned(),
            nonce: client_nonce(),
            supports: supports
                .iter()
                .map(|word| Part::parse(word).expect(word))
                .collect(),
        }
        .encode()
    }

    fn challenged(hub: &mut Hub, id: ConnectionId, supports: &[&str]) -> Nonce {
        match &hub.receive(id, &hello(1, ADDRESS, supports), now())[..] {
            [
                Outgoing {
                    message: ServerMessage::Challenge { nonce, .. },
                    ..
                },
            ] => *nonce,
            other => panic!("the server sends a challenge, not {other:?}"),
        }
    }

    fn credentials(hub: &Hub, person: u8, address: &str, server: &Nonce) -> (Key, Signature) {
        let account = seed(person).account_key(&hub.key.public());
        let client = client_nonce();
        let exchange = Exchange {
            address,
            client: &client,
            server,
        };
        (
            account.public(),
            account.prove(&hub.key.public(), &exchange),
        )
    }

    fn identify(hub: &Hub, person: u8, server: &Nonce, invitation: Option<&Code>) -> String {
        let (key, proof) = credentials(hub, person, ADDRESS, server);
        ClientMessage::Identify {
            reference: Some(2),
            key,
            proof,
            invitation: invitation.cloned(),
        }
        .encode()
    }

    fn register(hub: &Hub, person: u8, server: &Nonce, who: &str, invitation: &Code) -> String {
        let (key, proof) = credentials(hub, person, ADDRESS, server);
        ClientMessage::Register {
            reference: Some(2),
            key,
            proof,
            name: who.to_owned(),
            invitation: invitation.clone(),
        }
        .encode()
    }

    fn welcome(who: &str, role: Role) -> ServerMessage {
        ServerMessage::Welcome {
            reference: Some(2),
            name: name(who),
            role,
        }
    }

    fn owner_code(hub: &mut Hub) -> Code {
        let invite = hub
            .owner_link(now())
            .expect("stored")
            .expect("the server has no owner");
        Code::parse(invite.rsplit('/').next().expect("a invite has a code")).expect("a valid code")
    }

    fn invitation(hub: &mut Hub, number: u32) -> Code {
        let owner = seed(1).account_key(&hub.key.public()).public();
        let owner = hub
            .rules
            .account(owner.as_bytes())
            .expect("the owner has an account")
            .id;
        hub.rules
            .invite(&mut hub.store, owner, &code(number), None, None, now())
            .expect("stored")
            .expect("the owner can invite")
            .code
    }

    fn new_owner_code(hub: &mut Hub, number: u32) -> Code {
        hub.rules
            .new_owner_code(&mut hub.store, &code(number), now())
            .expect("stored")
            .expect("the code is new");
        code(number)
    }

    fn registered(hub: &mut Hub, person: u8, who: &str, invitation: &Code) -> ConnectionId {
        let id = hub.connect();
        let server = challenged(hub, id, &[]);
        let line = register(hub, person, &server, who, invitation);
        let outgoing = hub.receive(id, &line, now());
        assert!(
            matches!(outgoing[0].message, ServerMessage::Welcome { .. }),
            "{outgoing:?}"
        );
        id
    }

    fn with_owner() -> (Hub, ConnectionId) {
        let mut hub = new_hub();
        let invite = owner_code(&mut hub);
        let ana = registered(&mut hub, 1, "ana", &invite);
        (hub, ana)
    }

    #[test]
    fn answers_a_hello_with_a_challenge_that_proves_the_server_key() {
        let mut hub = new_hub();
        let id = hub.connect();

        let outgoing = hub.receive(id, &hello(7, ADDRESS, &["voice"]), now());

        let [
            Outgoing {
                to,
                message:
                    ServerMessage::Challenge {
                        reference,
                        protocol,
                        key,
                        nonce,
                        proof,
                        requires,
                    },
            },
        ] = &outgoing[..]
        else {
            panic!("the server sends one challenge, not {outgoing:?}");
        };
        assert_eq!((*to, *reference, *protocol), (id, Some(7), 0));
        assert_eq!(*key, hub.key.public());
        assert_eq!(*nonce, Nonce::from_bytes([1; 32]));
        assert_eq!(requires, &[]);
        let client = client_nonce();
        let exchange = Exchange {
            address: ADDRESS,
            client: &client,
            server: nonce,
        };
        assert!(exchange.server_proof_is_valid(key, proof));
    }

    #[test]
    fn makes_a_different_random_value_for_each_connection() {
        let mut hub = new_hub();
        let first = hub.connect();
        let second = hub.connect();

        assert_ne!(
            challenged(&mut hub, first, &[]),
            challenged(&mut hub, second, &[])
        );
    }

    #[test]
    fn refuses_a_hello_that_it_cannot_accept() {
        let mut hub = new_hub();
        let id = hub.connect();
        let other_version = ClientMessage::Hello {
            reference: Some(1),
            protocol: 1,
            address: ADDRESS.to_owned(),
            nonce: client_nonce(),
            supports: Vec::new(),
        }
        .encode();

        assert_eq!(
            hub.receive(id, &other_version, now()),
            [to(id, error(Some(1), ErrorCode::UnsupportedProtocol))]
        );
        for address in [
            "false.example.org:443",
            "chat.example.org:444",
            "chat.example.org",
            "",
        ] {
            assert_eq!(
                hub.receive(id, &hello(4, address, &[]), now()),
                [to(id, error(Some(4), ErrorCode::WrongAddress))],
                "{address}"
            );
        }

        challenged(&mut hub, id, &[]);
        assert_eq!(
            hub.receive(id, &hello(5, ADDRESS, &[]), now()),
            [to(id, error(Some(5), ErrorCode::DuplicateHello))]
        );
        assert_eq!(
            hub.receive(id, &guest(6, "ana"), now()),
            [to(id, error(Some(6), ErrorCode::DuplicateHello))]
        );
    }

    #[test]
    fn gives_an_owner_link_with_the_address_of_the_server() {
        let mut hub = new_hub();

        let first = hub.owner_link(now()).expect("stored");
        let second = hub.owner_link(later()).expect("stored");

        assert_eq!(
            first.as_deref(),
            Some("https://chat.example.org/i/code00000001")
        );
        assert_eq!(second, first);
    }

    #[test]
    fn makes_the_user_of_the_owner_link_the_server_owner() {
        let mut hub = new_hub();
        let invite = owner_code(&mut hub);
        let id = hub.connect();
        let server = challenged(&mut hub, id, &[]);

        let line = register(&hub, 1, &server, "ana", &invite);
        let outgoing = hub.receive(id, &line, now());

        assert_eq!(
            outgoing,
            [to(id, welcome("ana", Role::Owner)), to(id, join("ana"))]
        );
        assert_eq!(hub.owner_link(now()).expect("stored"), None);
    }

    #[test]
    fn makes_an_invited_user_a_member_and_tells_the_lobby() {
        let (mut hub, ana) = with_owner();
        let code = invitation(&mut hub, 50);
        let ben = hub.connect();
        let server = challenged(&mut hub, ben, &[]);

        let line = register(&hub, 2, &server, "ben", &code);
        let outgoing = hub.receive(ben, &line, now());

        assert_eq!(
            outgoing,
            [
                to(ben, welcome("ben", Role::Member)),
                to(ana, join("ben")),
                to(ben, join("ben")),
            ]
        );
    }

    #[test]
    fn signs_in_a_user_who_has_an_account() {
        let (mut hub, ana) = with_owner();
        hub.disconnect(ana, now());
        let id = hub.connect();
        let server = challenged(&mut hub, id, &[]);

        let line = identify(&hub, 1, &server, None);
        let outgoing = hub.receive(id, &line, now());

        assert_eq!(
            outgoing,
            [to(id, welcome("ana", Role::Owner)), to(id, join("ana"))]
        );
    }

    #[test]
    fn refuses_a_key_that_has_no_account() {
        let (mut hub, _ana) = with_owner();
        let id = hub.connect();
        let server = challenged(&mut hub, id, &[]);

        let line = identify(&hub, 2, &server, None);

        assert_eq!(
            hub.receive(id, &line, now()),
            [to(id, error(Some(2), ErrorCode::UnknownAccount))]
        );
    }

    #[test]
    fn refuses_a_sign_in_message_at_an_incorrect_moment() {
        let (mut hub, ana) = with_owner();
        let invite = code(1);
        let new = hub.connect();

        for line in [
            identify(&hub, 1, &client_nonce(), None),
            register(&hub, 2, &client_nonce(), "ben", &invite),
        ] {
            assert_eq!(
                hub.receive(new, &line, now()),
                [to(new, error(Some(2), ErrorCode::HelloRequired))]
            );
            assert_eq!(
                hub.receive(ana, &line, now()),
                [to(ana, error(Some(2), ErrorCode::DuplicateIdentify))]
            );
        }

        challenged(&mut hub, new, &[]);
        assert_eq!(
            hub.receive(new, &send(Some(8), LOBBY, "Hello"), now()),
            [to(new, error(Some(8), ErrorCode::IdentifyRequired))]
        );
    }

    #[test]
    fn refuses_a_proof_that_is_not_for_this_connection() {
        let (mut hub, ana) = with_owner();
        hub.disconnect(ana, now());
        let invite = invitation(&mut hub, 50);
        let id = hub.connect();
        let server = challenged(&mut hub, id, &[]);
        let earlier = Nonce::from_bytes([1; 32]);
        assert_ne!(server, earlier);

        let (key, _) = credentials(&hub, 1, ADDRESS, &server);
        let (_, for_other_address) = credentials(&hub, 1, "false.example.org:443", &server);
        let (_, from_other_seed) = credentials(&hub, 2, ADDRESS, &server);
        let lines = [
            identify(&hub, 1, &earlier, None),
            register(&hub, 2, &earlier, "ben", &invite),
            ClientMessage::Identify {
                reference: Some(2),
                key,
                proof: for_other_address,
                invitation: None,
            }
            .encode(),
            ClientMessage::Identify {
                reference: Some(2),
                key,
                proof: from_other_seed,
                invitation: None,
            }
            .encode(),
        ];
        for line in lines {
            assert_eq!(
                hub.receive(id, &line, now()),
                [to(id, error(Some(2), ErrorCode::InvalidProof))],
                "{line}"
            );
        }

        let line = identify(&hub, 1, &server, None);
        assert_eq!(
            hub.receive(id, &line, now())[0],
            to(id, welcome("ana", Role::Owner))
        );
    }

    #[test]
    fn refuses_a_new_account_that_breaks_a_rule() {
        let (mut hub, _ana) = with_owner();
        let open = invitation(&mut hub, 50);
        let id = hub.connect();
        let server = challenged(&mut hub, id, &[]);

        let attempts = [
            (2, "Not A Name", open.clone(), ErrorCode::InvalidName),
            (2, "ben", code(99), ErrorCode::InvalidInvitation),
            (2, "ben", code(1), ErrorCode::InvalidInvitation),
            (2, "ana", open.clone(), ErrorCode::NameTaken),
            (1, "ana-2", open.clone(), ErrorCode::AccountExists),
        ];
        for (person, who, invitation, refused) in attempts {
            let line = register(&hub, person, &server, who, &invitation);
            assert_eq!(
                hub.receive(id, &line, now()),
                [to(id, error(Some(2), refused))],
                "{who}"
            );
        }

        let line = register(&hub, 2, &server, "ben", &open);
        assert_eq!(
            hub.receive(id, &line, now())[0],
            to(id, welcome("ben", Role::Member))
        );
    }

    #[test]
    fn makes_an_account_that_exists_the_owner_with_a_new_owner_link() {
        let (mut hub, ana) = with_owner();
        let open = invitation(&mut hub, 50);
        let ben = registered(&mut hub, 2, "ben", &open);
        hub.disconnect(ana, now());
        hub.disconnect(ben, now());
        new_owner_code(&mut hub, 60);

        let id = hub.connect();
        let server = challenged(&mut hub, id, &[]);
        let line = identify(&hub, 2, &server, Some(&code(60)));
        assert_eq!(
            hub.receive(id, &line, now())[0],
            to(id, welcome("ben", Role::Owner))
        );

        let id = hub.connect();
        let server = challenged(&mut hub, id, &[]);
        let line = identify(&hub, 1, &server, Some(&code(60)));
        assert_eq!(
            hub.receive(id, &line, now())[0],
            to(id, welcome("ana", Role::Member))
        );
    }

    #[test]
    fn ignores_an_invitation_at_the_sign_in_of_a_member() {
        let (mut hub, ana) = with_owner();
        let open = invitation(&mut hub, 50);
        let ben = registered(&mut hub, 2, "ben", &open);
        hub.disconnect(ben, now());

        let id = hub.connect();
        let server = challenged(&mut hub, id, &[]);
        let line = identify(&hub, 2, &server, Some(&open));
        let outgoing = hub.receive(id, &line, now());

        assert_eq!(
            outgoing,
            [
                to(id, welcome("ben", Role::Member)),
                to(ana, join("ben")),
                to(id, join("ben")),
            ]
        );
        assert_eq!(hub.rules.open_invitations(now())[0].used, 1);
    }

    #[test]
    fn refuses_a_client_that_does_not_support_a_required_part() {
        let store = Store::in_memory().expect("a database in memory");
        let requires = vec![Part::parse("sealed").expect("valid")];
        let settings = Settings {
            requires: requires.clone(),
            ..settings()
        };
        let mut hub = Hub::new(store, settings, random()).expect("a hub");
        let invite = owner_code(&mut hub);

        let id = hub.connect();
        match &hub.receive(id, &hello(1, ADDRESS, &[]), now())[..] {
            [
                Outgoing {
                    message: ServerMessage::Challenge { requires: sent, .. },
                    ..
                },
            ] => assert_eq!(sent, &requires),
            other => panic!("{other:?}"),
        }
        let server = Nonce::from_bytes([1; 32]);
        let line = register(&hub, 1, &server, "ana", &invite);
        assert_eq!(
            hub.receive(id, &line, now()),
            [to(id, error(Some(2), ErrorCode::MissingPart))]
        );

        let id = hub.connect();
        let server = challenged(&mut hub, id, &["voice", "sealed"]);
        let line = register(&hub, 1, &server, "ana", &invite);
        assert_eq!(
            hub.receive(id, &line, now())[0],
            to(id, welcome("ana", Role::Owner))
        );
        hub.disconnect(id, now());

        let id = hub.connect();
        let server = challenged(&mut hub, id, &["voice"]);
        let line = identify(&hub, 1, &server, None);
        assert_eq!(
            hub.receive(id, &line, now()),
            [to(id, error(Some(2), ErrorCode::MissingPart))]
        );
    }

    #[test]
    fn tells_the_lobby_one_time_about_a_user_with_two_connections() {
        let (mut hub, first) = with_owner();
        let open = invitation(&mut hub, 50);
        let ben = registered(&mut hub, 2, "ben", &open);

        let second = hub.connect();
        let server = challenged(&mut hub, second, &[]);
        let line = identify(&hub, 1, &server, None);
        assert_eq!(
            hub.receive(second, &line, now()),
            [to(second, welcome("ana", Role::Owner))]
        );

        let delivered = hub.receive(second, &send(Some(4), LOBBY, "Hello"), now());
        let receivers: Vec<ConnectionId> = delivered.iter().map(|outgoing| outgoing.to).collect();
        assert_eq!(receivers, [first, ben, second]);

        assert_eq!(hub.disconnect(first, later()), []);
        assert_eq!(
            hub.disconnect(second, later()),
            [to(
                ben,
                ServerMessage::Leave {
                    channel: LOBBY.to_owned(),
                    name: name("ana"),
                    time: later(),
                }
            )]
        );
    }

    #[test]
    fn answers_with_a_server_error_when_the_storage_fails() {
        let (mut hub, ana) = with_owner();
        let open = invitation(&mut hub, 50);
        let link = new_owner_code(&mut hub, 60);
        hub.disconnect(ana, now());
        let id = hub.connect();
        let server = challenged(&mut hub, id, &[]);
        hub.store.lose_its_tables();

        for line in [
            identify(&hub, 1, &server, Some(&link)),
            register(&hub, 2, &server, "ben", &open),
        ] {
            assert_eq!(
                hub.receive(id, &line, now()),
                [to(id, error(Some(2), ErrorCode::ServerError))]
            );
        }
    }

    #[test]
    fn needs_no_storage_for_a_sign_in_that_changes_nothing() {
        let (mut hub, ana) = with_owner();
        hub.disconnect(ana, now());
        let id = hub.connect();
        let server = challenged(&mut hub, id, &[]);
        hub.store.lose_its_tables();

        let line = identify(&hub, 1, &server, None);

        assert_eq!(
            hub.receive(id, &line, now())[0],
            to(id, welcome("ana", Role::Owner))
        );
    }

    #[test]
    fn reports_an_owner_link_that_it_cannot_store() {
        let mut hub = new_hub();
        hub.store.lose_its_tables();

        assert!(matches!(
            hub.owner_link(now()),
            Err(StoreError::Database(_))
        ));
    }

    #[test]
    fn reports_a_new_code_that_an_invitation_has() {
        let mut store = Store::in_memory().expect("a database in memory");
        store
            .write(&[Change::PutInvitation(stored_invitation(1, Some(5), 0))])
            .expect("stored");
        let mut hub = Hub::new(store, settings(), random()).expect("a hub");

        assert!(matches!(hub.owner_link(now()), Err(StoreError::CodeInUse)));
    }

    #[test]
    fn does_not_start_with_data_that_breaks_a_rule() {
        let mut store = Store::in_memory().expect("a database in memory");
        store
            .write(&[Change::PutInvitation(stored_invitation(1, Some(1), 2))])
            .expect("stored");

        let result = Hub::new(store, settings(), random());

        assert!(matches!(result, Err(StoreError::BrokenRules)));
    }

    fn stored_invitation(number: u32, limit: Option<u32>, used: u32) -> Invitation {
        Invitation {
            code: code(number),
            owner: false,
            maker: None,
            created: now(),
            end: None,
            uses: limit,
            used,
            revoked: None,
        }
    }
}

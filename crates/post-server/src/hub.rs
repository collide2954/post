use std::collections::BTreeMap;
use std::num::NonZeroU32;

use post_crypto::{Exchange, ServerKey};
use post_proto::{
    ClientMessage, Code, ErrorCode, Invitation, Key, LOBBY, Name, Nonce, PROTOCOL_VERSION, Part,
    Role, ServerMessage, Signature, TextError, Timestamp, check_text,
};

use post_rules::{
    Account, AccountId, InviteError, Limits, RegisterError, RenameError, RevokeError, Rules, later,
};

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
    account: AccountId,
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
        Ok(code.map(|code| self.link(&code)))
    }

    #[must_use]
    pub fn open_link(&self, code: &Code, now: Timestamp) -> Option<String> {
        self.rules.code_is_open(code, now).then(|| self.link(code))
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

            ClientMessage::Send {
                reference,
                channel,
                text,
            } => (
                reference,
                examine_text(&state, &channel, &text)
                    .map(|member| self.deliver(from, reference, &member.name, &text, now)),
            ),
            ClientMessage::Invite {
                reference,
                lifetime,
                uses,
            } => (
                reference,
                self.invite(from, reference, &state, lifetime, uses, now),
            ),
            ClientMessage::Invitations { reference } => (
                reference,
                self.list_invitations(from, reference, &state, now),
            ),
            ClientMessage::Revoke { reference, code } => {
                (reference, self.revoke(from, reference, &state, &code, now))
            }
            ClientMessage::Rename { reference, name } => {
                (reference, self.rename(from, reference, &state, &name, now))
            }
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

    fn invite(
        &mut self,
        from: ConnectionId,
        reference: Option<u32>,
        state: &State,
        lifetime: Option<NonZeroU32>,
        uses: Option<NonZeroU32>,
        now: Timestamp,
    ) -> Result<Vec<Outgoing>, Refused> {
        let account = account_of(state)?;
        let end = lifetime.map(|seconds| later(now, u64::from(seconds.get()) * 1000));
        let code = (self.random.code)();
        self.rules
            .invite(
                &mut self.store,
                account,
                &code,
                end,
                uses.map(NonZeroU32::get),
                now,
            )
            .map_err(|_| ErrorCode::ServerError)?
            .map_err(|error| match error {
                InviteError::NotPermitted => ErrorCode::NotPermitted,
                InviteError::CodeTaken => ErrorCode::ServerError,
            })?;
        Ok(self.invitations(from, reference, now))
    }

    fn list_invitations(
        &self,
        from: ConnectionId,
        reference: Option<u32>,
        state: &State,
        now: Timestamp,
    ) -> Result<Vec<Outgoing>, Refused> {
        if !self.rules.is_owner(account_of(state)?) {
            return Err(ErrorCode::NotPermitted);
        }
        Ok(self.invitations(from, reference, now))
    }

    fn revoke(
        &mut self,
        from: ConnectionId,
        reference: Option<u32>,
        state: &State,
        code: &Code,
        now: Timestamp,
    ) -> Result<Vec<Outgoing>, Refused> {
        let account = account_of(state)?;
        self.rules
            .revoke(&mut self.store, account, code, now)
            .map_err(|_| ErrorCode::ServerError)?
            .map_err(|error| match error {
                RevokeError::NotPermitted => ErrorCode::NotPermitted,
                RevokeError::NotOpen => ErrorCode::InvalidInvitation,
            })?;
        Ok(self.invitations(from, reference, now))
    }

    fn invitations(
        &self,
        to: ConnectionId,
        reference: Option<u32>,
        now: Timestamp,
    ) -> Vec<Outgoing> {
        let list = self
            .rules
            .open_invitations(now)
            .into_iter()
            .rev()
            .map(|open| Invitation {
                link: self.link(&open.code),
                code: open.code,
                created: open.created,
                end: open.end,
                uses: open.uses,
                used: open.used,
            })
            .collect();
        let message = ServerMessage::Invitations { reference, list };
        vec![Outgoing { to, message }]
    }

    fn rename(
        &mut self,
        from: ConnectionId,
        reference: Option<u32>,
        state: &State,
        wanted: &str,
        now: Timestamp,
    ) -> Result<Vec<Outgoing>, Refused> {
        let account = account_of(state)?;
        let new = Name::parse(wanted).map_err(|_| ErrorCode::InvalidName)?;
        let old = self
            .rules
            .rename(&mut self.store, account, &new, now)
            .map_err(|_| ErrorCode::ServerError)?
            .map_err(|error| match error {
                RenameError::TooSoon => ErrorCode::TooSoon,
                RenameError::NameTaken => ErrorCode::NameTaken,
                RenameError::UnknownAccount => ErrorCode::ServerError,
            })?;
        for state in self.connections.values_mut() {
            if let State::Present(member) = state
                && member.account == account
            {
                member.name = new.clone();
            }
        }
        Ok(self.to_each_member(|to| ServerMessage::Rename {
            reference: reference.filter(|_| to == from),
            old: old.clone(),
            new: new.clone(),
            time: now,
        }))
    }

    fn link(&self, code: &Code) -> String {
        format!("{}/i/{code}", self.settings.link)
    }

    fn has_a_connection(&self, member: &Member) -> bool {
        self.members()
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
            account: account.id,
            name: account.name.clone(),
            role: if account.owner {
                Role::Owner
            } else {
                Role::Member
            },
        }
    }
}

fn present(state: &State) -> Result<&Member, Refused> {
    match state {
        State::New => Err(ErrorCode::HelloRequired),
        State::Challenged(_) => Err(ErrorCode::IdentifyRequired),
        State::Present(member) => Ok(member),
    }
}

fn account_of(state: &State) -> Result<AccountId, Refused> {
    Ok(present(state)?.account)
}

fn examine_text<'a>(state: &'a State, channel: &str, text: &str) -> Result<&'a Member, Refused> {
    let member = present(state)?;
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
mod tests;

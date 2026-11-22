use std::collections::BTreeSet;

use post_proto::{ChannelKind, ChannelName, ErrorCode, ListedChannel, ServerMessage};
use post_rules::{AccountId, Channel, ChannelLimits, CreateError, JoinError, Kind, LeaveError};

use super::{
    Answer, ConnectionId, DAY, Hub, Outgoing, Refused, Request, account_of, examine_text, present,
};

pub const CHANNEL_LIMITS: ChannelLimits = ChannelLimits {
    owned: 10,
    rename_interval: 30 * DAY,
    name_reserved: 30 * DAY,
};

const LISTED_IN_ONE_MESSAGE: usize = 40;

impl Hub {
    pub(super) fn create_channel(
        &mut self,
        request: &Request<'_>,
        channel: &str,
        kind: ChannelKind,
    ) -> Answer {
        let account = account_of(request.state)?;
        let name = ChannelName::parse(channel).map_err(|_| ErrorCode::InvalidChannelName)?;

        let kind = match kind {
            ChannelKind::Public => Kind::Public,
            ChannelKind::Private => Kind::Private,
        };
        let id = self
            .channels
            .create(&mut self.store, account, &name, kind, request.now)
            .map_err(|_| ErrorCode::ServerError)?
            .map_err(|error| match error {
                CreateError::NameTaken => ErrorCode::NameTaken,
                CreateError::TooMany => ErrorCode::TooManyChannels,
                CreateError::Full => ErrorCode::ServerError,
            })?;
        let channel = self.channels.by_id(id).ok_or(ErrorCode::ServerError)?;
        Ok(self.states_for(channel, account, request))
    }

    pub(super) fn join_channel(&mut self, request: &Request<'_>, channel: &str) -> Answer {
        let member = present(request.state)?;
        let found = self.visible(member.account, channel)?;
        let (id, name) = (found.id, found.name.clone());
        self.channels
            .join(&mut self.store, member.account, id, request.now)
            .map_err(|_| ErrorCode::ServerError)?
            .map_err(|error| match error {
                JoinError::AlreadyMember => ErrorCode::AlreadyMember,
                JoinError::NoSuchChannel | JoinError::NotInvited => ErrorCode::NoSuchChannel,
            })?;
        let channel = self.channels.by_id(id).ok_or(ErrorCode::ServerError)?;
        let mut outgoing = self.states_for(channel, member.account, request);
        outgoing.extend(self.to_each_member_of(channel, |_| ServerMessage::Join {
            channel: name.clone(),
            name: member.name.clone(),
            time: request.now,
        }));
        Ok(outgoing)
    }

    pub(super) fn leave_channel(&mut self, request: &Request<'_>, channel: &str) -> Answer {
        let member = present(request.state)?;
        let found = self.visible(member.account, channel)?;
        let (id, name) = (found.id, found.name.clone());
        let receivers: Vec<ConnectionId> = self.connections_in(found).collect();
        self.channels
            .leave(&mut self.store, member.account, id)
            .map_err(|_| ErrorCode::ServerError)?
            .map_err(|error| match error {
                LeaveError::NoSuchChannel => ErrorCode::NoSuchChannel,
                LeaveError::NotMember => ErrorCode::NotMember,
                LeaveError::Owner => ErrorCode::OwnerCannotLeave,
            })?;
        Ok(receivers
            .into_iter()
            .map(|to| Outgoing {
                to,
                message: ServerMessage::Leave {
                    reference: request.reference.filter(|_| to == request.from),
                    channel: name.clone(),
                    name: member.name.clone(),
                    time: request.now,
                },
            })
            .collect())
    }

    pub(super) fn send_text(&self, request: &Request<'_>, channel: &str, text: &str) -> Answer {
        let member = present(request.state)?;
        let channel = self.visible(member.account, channel)?;
        if !channel.is_member(member.account) {
            return Err(ErrorCode::NotMember);
        }
        examine_text(text)?;
        Ok(
            self.to_each_member_of(channel, |to| ServerMessage::Message {
                reference: request.reference.filter(|_| to == request.from),
                channel: channel.name.clone(),
                from: member.name.clone(),
                time: request.now,
                text: text.to_owned(),
            }),
        )
    }

    pub(super) fn connections_that_see(&self, account: AccountId) -> BTreeSet<ConnectionId> {
        let (channels, _) = self.channels.rows();
        let mut connections: BTreeSet<ConnectionId> = self.connections_of(account).collect();
        for channel in channels {
            if channel.is_member(account) {
                connections.extend(self.connections_in(channel));
            }
        }
        connections
    }

    pub(super) fn list_channels(&self, request: &Request<'_>) -> Answer {
        let account = account_of(request.state)?;
        let (channels, _) = self.channels.rows();
        let mut list: Vec<ListedChannel> = channels
            .iter()
            .filter(|channel| !channel.kind.is_private() || channel.is_invited(account))
            .map(|channel| ListedChannel {
                channel: channel.name.clone(),
                kind: kind_of(channel),
                topic: channel.topic.clone(),
                members: u32::try_from(channel.members.len()).unwrap_or(u32::MAX),
            })
            .collect();
        list.sort_by(|first, second| first.channel.cmp(&second.channel));
        let mut parts: Vec<Vec<ListedChannel>> = list
            .chunks(LISTED_IN_ONE_MESSAGE)
            .map(<[ListedChannel]>::to_vec)
            .collect();
        if parts.is_empty() {
            parts.push(Vec::new());
        }
        let count = parts.len();
        Ok(parts
            .into_iter()
            .enumerate()
            .map(|(part, list)| Outgoing {
                to: request.from,
                message: ServerMessage::Channels {
                    reference: request.reference,
                    list,
                    more: part + 1 < count,
                },
            })
            .collect())
    }

    pub(super) fn visible(&self, account: AccountId, channel: &str) -> Result<&Channel, Refused> {
        let name = ChannelName::parse(channel).map_err(|_| ErrorCode::NoSuchChannel)?;
        let channel = self
            .channels
            .by_name(&name)
            .ok_or(ErrorCode::NoSuchChannel)?;
        let is_hidden = channel.kind.is_private()
            && !channel.is_member(account)
            && !channel.is_invited(account);
        if is_hidden {
            return Err(ErrorCode::NoSuchChannel);
        }
        Ok(channel)
    }

    fn states_for(
        &self,
        channel: &Channel,
        account: AccountId,
        request: &Request<'_>,
    ) -> Vec<Outgoing> {
        self.connections_of(account)
            .filter_map(|to| {
                let reference = request.reference.filter(|_| to == request.from);
                self.state_of(channel, reference)
                    .map(|message| Outgoing { to, message })
            })
            .collect()
    }

    fn connections_in(&self, channel: &Channel) -> impl Iterator<Item = ConnectionId> {
        channel
            .members
            .iter()
            .flat_map(move |member| self.connections_of(member.account))
    }

    pub(super) fn to_each_member_of(
        &self,
        channel: &Channel,
        message: impl Fn(ConnectionId) -> ServerMessage,
    ) -> Vec<Outgoing> {
        self.connections_in(channel)
            .map(|to| Outgoing {
                to,
                message: message(to),
            })
            .collect()
    }

    pub(super) fn channel_states(
        &self,
        to: ConnectionId,
        account: AccountId,
    ) -> impl Iterator<Item = Outgoing> {
        let (channels, _) = self.channels.rows();
        channels
            .iter()
            .filter(move |channel| channel.is_member(account))
            .filter_map(move |channel| self.state_of(channel, None))
            .map(move |message| Outgoing { to, message })
    }

    fn state_of(&self, channel: &Channel, reference: Option<u32>) -> Option<ServerMessage> {
        Some(ServerMessage::Channel {
            reference,
            channel: channel.name.clone(),
            kind: kind_of(channel),
            topic: channel.topic.clone(),
            owner: self.rules.name_of(channel.owner)?.clone(),
            created: channel.created,
        })
    }

    fn connections_of(&self, account: AccountId) -> impl Iterator<Item = ConnectionId> {
        self.online.get(&account).into_iter().flatten().copied()
    }
}

fn kind_of(channel: &Channel) -> ChannelKind {
    if channel.kind.is_private() {
        ChannelKind::Private
    } else {
        ChannelKind::Public
    }
}

use post_proto::{ChannelKind, ChannelName, ErrorCode, LOBBY, ServerMessage};
use post_rules::{AccountId, Channel, ChannelLimits, CreateError, JoinError, Kind, LeaveError};

use super::{Answer, ConnectionId, DAY, Hub, Outgoing, Refused, Request, account_of, present};

pub const CHANNEL_LIMITS: ChannelLimits = ChannelLimits {
    owned: 10,
    rename_interval: 30 * DAY,
    name_reserved: 30 * DAY,
};

impl Hub {
    pub(super) fn create_channel(
        &mut self,
        request: &Request<'_>,
        channel: &str,
        kind: ChannelKind,
    ) -> Answer {
        let account = account_of(request.state)?;
        let name = ChannelName::parse(channel).map_err(|_| ErrorCode::InvalidChannelName)?;
        if name.as_str() == LOBBY {
            return Err(ErrorCode::NameTaken);
        }
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
            channel: name.as_str().to_owned(),
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
                    channel: name.as_str().to_owned(),
                    name: member.name.clone(),
                    time: request.now,
                },
            })
            .collect())
    }

    fn visible(&self, account: AccountId, channel: &str) -> Result<&Channel, Refused> {
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

    fn to_each_member_of(
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
            kind: if channel.kind.is_private() {
                ChannelKind::Private
            } else {
                ChannelKind::Public
            },
            topic: channel.topic.clone(),
            owner: self.rules.name_of(channel.owner)?.clone(),
            created: channel.created,
        })
    }

    fn connections_of(&self, account: AccountId) -> impl Iterator<Item = ConnectionId> {
        self.online.get(&account).into_iter().flatten().copied()
    }
}

use post_proto::{ChannelKind, ChannelName, ErrorCode, LOBBY, ServerMessage};
use post_rules::{AccountId, Channel, ChannelLimits, CreateError, Kind};

use super::{Answer, ConnectionId, DAY, Hub, Outgoing, Request, account_of};

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
        Ok(self
            .connections_of(account)
            .filter_map(|to| {
                let reference = request.reference.filter(|_| to == request.from);
                self.state_of(channel, reference)
                    .map(|message| Outgoing { to, message })
            })
            .collect())
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

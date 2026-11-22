use post_proto::{ErrorCode, Name, ServerMessage};
use post_rules::{AccountId, Channel, ChannelInviteError, InvitationError, Invited, RefuseError};

use super::{Answer, ConnectionId, Hub, Outgoing, Request, present};

impl Hub {
    pub(super) fn invite_to_channel(
        &mut self,
        request: &Request<'_>,
        channel: &str,
        wanted: &str,
    ) -> Answer {
        let member = present(request.state)?;
        let found = self.visible(member.account, channel)?;
        if found.owner != member.account {
            return Err(ErrorCode::NotPermitted);
        }
        let id = found.id;
        let name = Name::parse(wanted).map_err(|_| ErrorCode::NoSuchUser)?;
        let account = self.rules.id_of(&name).ok_or(ErrorCode::NoSuchUser)?;
        self.channels
            .invite(&mut self.store, member.account, id, account, request.now)
            .map_err(|_| ErrorCode::ServerError)?
            .map_err(|error| match error {
                ChannelInviteError::NoSuchChannel => ErrorCode::NoSuchChannel,
                ChannelInviteError::NotPermitted => ErrorCode::NotPermitted,
                ChannelInviteError::NotPrivate => ErrorCode::NotPrivate,
                ChannelInviteError::AlreadyMember => ErrorCode::AlreadyMember,
                ChannelInviteError::AlreadyInvited => ErrorCode::AlreadyInvited,
            })?;
        let channel = self.channels.by_id(id).ok_or(ErrorCode::ServerError)?;
        Ok(
            self.to_the_two(request, [member.account, account], |reference| {
                ServerMessage::ChannelInvite {
                    reference,
                    channel: channel.name.clone(),
                    name: name.clone(),
                    owner: member.name.clone(),
                    time: request.now,
                }
            }),
        )
    }

    pub(super) fn cancel_invitation(
        &mut self,
        request: &Request<'_>,
        channel: &str,
        wanted: &str,
    ) -> Answer {
        let member = present(request.state)?;
        let found = self.visible(member.account, channel)?;
        if found.owner != member.account {
            return Err(ErrorCode::NotPermitted);
        }
        let (id, title) = (found.id, found.name.clone());
        let name = Name::parse(wanted).map_err(|_| ErrorCode::NoSuchUser)?;
        let account = self.rules.id_of(&name).ok_or(ErrorCode::NoSuchUser)?;
        self.channels
            .cancel(&mut self.store, member.account, id, account)
            .map_err(|_| ErrorCode::ServerError)?
            .map_err(|error| match error {
                InvitationError::NoSuchChannel => ErrorCode::NoSuchChannel,
                InvitationError::NotPermitted => ErrorCode::NotPermitted,
                InvitationError::NotInvited => ErrorCode::NotInvited,
            })?;
        Ok(
            self.to_the_two(request, [member.account, account], |reference| {
                ServerMessage::ChannelCancel {
                    reference,
                    channel: title.clone(),
                    name: name.clone(),
                    time: request.now,
                }
            }),
        )
    }

    pub(super) fn refuse_invitation(&mut self, request: &Request<'_>, channel: &str) -> Answer {
        let member = present(request.state)?;
        let found = self.visible(member.account, channel)?;
        let (id, owner, title) = (found.id, found.owner, found.name.clone());
        self.channels
            .refuse(&mut self.store, member.account, id)
            .map_err(|_| ErrorCode::ServerError)?
            .map_err(|error| match error {
                RefuseError::NoSuchChannel => ErrorCode::NoSuchChannel,
                RefuseError::NotInvited => ErrorCode::NotInvited,
            })?;
        Ok(
            self.to_the_two(request, [owner, member.account], |reference| {
                ServerMessage::ChannelRefuse {
                    reference,
                    channel: title.clone(),
                    name: member.name.clone(),
                    time: request.now,
                }
            }),
        )
    }

    pub(super) fn invitation_states(&self, to: ConnectionId, account: AccountId) -> Vec<Outgoing> {
        let (channels, _) = self.channels.rows();
        channels
            .iter()
            .flat_map(|channel| {
                channel
                    .invited
                    .iter()
                    .filter(move |invited| invited.account == account || channel.owner == account)
                    .filter_map(move |invited| self.invitation_message(channel, invited))
            })
            .map(|message| Outgoing { to, message })
            .collect()
    }

    pub(super) fn invitations_of(&self, channel: &Channel) -> Vec<Outgoing> {
        let mut outgoing = Vec::new();
        for invited in &channel.invited {
            let Some(message) = self.invitation_message(channel, invited) else {
                continue;
            };
            let receivers = self
                .connections_of(channel.owner)
                .chain(self.connections_of(invited.account));
            outgoing.extend(receivers.map(|to| Outgoing {
                to,
                message: message.clone(),
            }));
        }
        outgoing
    }

    fn invitation_message(&self, channel: &Channel, invited: &Invited) -> Option<ServerMessage> {
        Some(ServerMessage::ChannelInvite {
            reference: None,
            channel: channel.name.clone(),
            name: self.rules.name_of(invited.account)?.clone(),
            owner: self.rules.name_of(channel.owner)?.clone(),
            time: invited.created,
        })
    }

    fn to_the_two(
        &self,
        request: &Request<'_>,
        accounts: [AccountId; 2],
        message: impl Fn(Option<u32>) -> ServerMessage,
    ) -> Vec<Outgoing> {
        accounts
            .into_iter()
            .flat_map(|account| self.connections_of(account))
            .map(|to| Outgoing {
                to,
                message: message(request.reference.filter(|_| to == request.from)),
            })
            .collect()
    }
}

use post_proto::{ErrorCode, Name, ServerMessage};
use post_rules::{AccountId, ChannelInviteError};

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

    pub(super) fn invitation_states(&self, to: ConnectionId, account: AccountId) -> Vec<Outgoing> {
        let (channels, _) = self.channels.rows();
        let mut outgoing = Vec::new();
        for channel in channels {
            let Some(owner) = self.rules.name_of(channel.owner) else {
                continue;
            };
            for invitation in &channel.invited {
                if invitation.account != account && channel.owner != account {
                    continue;
                }
                let Some(name) = self.rules.name_of(invitation.account) else {
                    continue;
                };
                outgoing.push(Outgoing {
                    to,
                    message: ServerMessage::ChannelInvite {
                        reference: None,
                        channel: channel.name.clone(),
                        name: name.clone(),
                        owner: owner.clone(),
                        time: invitation.created,
                    },
                });
            }
        }
        outgoing
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

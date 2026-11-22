use post_proto::{ChannelName, ErrorCode, Name, ServerMessage, check_topic};
use post_rules::{ChannelRenameError, GiveError, OwnerError};

use super::{Answer, ConnectionId, Hub, Outgoing, Request, present};

impl Hub {
    pub(super) fn set_topic(
        &mut self,
        request: &Request<'_>,
        channel: &str,
        topic: &str,
    ) -> Answer {
        let member = present(request.state)?;
        let id = self.visible(member.account, channel)?.id;
        check_topic(topic).map_err(|_| ErrorCode::InvalidTopic)?;
        self.channels
            .set_topic(&mut self.store, member.account, id, topic)
            .map_err(|_| ErrorCode::ServerError)?
            .map_err(owner_error)?;
        let channel = self.channels.by_id(id).ok_or(ErrorCode::ServerError)?;
        Ok(
            self.to_each_member_of(channel, |to| ServerMessage::ChannelTopic {
                reference: request.reference.filter(|_| to == request.from),
                channel: channel.name.clone(),
                topic: topic.to_owned(),
                by: member.name.clone(),
                time: request.now,
            }),
        )
    }

    pub(super) fn rename_channel(
        &mut self,
        request: &Request<'_>,
        channel: &str,
        wanted: &str,
    ) -> Answer {
        let member = present(request.state)?;
        let id = self.visible(member.account, channel)?.id;
        let new = ChannelName::parse(wanted).map_err(|_| ErrorCode::InvalidChannelName)?;
        let old = self
            .channels
            .rename(&mut self.store, member.account, id, &new, request.now)
            .map_err(|_| ErrorCode::ServerError)?
            .map_err(|error| match error {
                ChannelRenameError::NoSuchChannel => ErrorCode::NoSuchChannel,
                ChannelRenameError::NotPermitted => ErrorCode::NotPermitted,
                ChannelRenameError::TooSoon => ErrorCode::TooSoon,
                ChannelRenameError::NameTaken => ErrorCode::NameTaken,
            })?;
        let channel = self.channels.by_id(id).ok_or(ErrorCode::ServerError)?;
        Ok(self
            .connections_that_know(channel)
            .map(|to| Outgoing {
                to,
                message: ServerMessage::ChannelRename {
                    reference: request.reference.filter(|_| to == request.from),
                    channel: old.clone(),
                    name: new.clone(),
                    time: request.now,
                },
            })
            .collect())
    }

    pub(super) fn give_channel(
        &mut self,
        request: &Request<'_>,
        channel: &str,
        receiver: &str,
    ) -> Answer {
        let member = present(request.state)?;
        let found = self.visible(member.account, channel)?;
        if found.owner != member.account {
            return Err(ErrorCode::NotPermitted);
        }
        let id = found.id;
        let name = Name::parse(receiver).map_err(|_| ErrorCode::NoSuchUser)?;
        let account = self.rules.id_of(&name).ok_or(ErrorCode::NoSuchUser)?;
        self.channels
            .give(&mut self.store, member.account, id, account)
            .map_err(|_| ErrorCode::ServerError)?
            .map_err(|error| match error {
                GiveError::NoSuchChannel => ErrorCode::NoSuchChannel,
                GiveError::NotPermitted => ErrorCode::NotPermitted,
                GiveError::NotMember => ErrorCode::NoSuchMember,
                GiveError::TooMany => ErrorCode::TooManyChannels,
            })?;
        let channel = self.channels.by_id(id).ok_or(ErrorCode::ServerError)?;
        let mut outgoing = self.to_each_member_of(channel, |to| ServerMessage::ChannelGive {
            reference: request.reference.filter(|_| to == request.from),
            channel: channel.name.clone(),
            name: name.clone(),
            time: request.now,
        });
        outgoing.extend(self.invitations_of(channel));
        Ok(outgoing)
    }

    pub(super) fn delete_channel(&mut self, request: &Request<'_>, channel: &str) -> Answer {
        let member = present(request.state)?;
        let found = self.visible(member.account, channel)?;
        let (id, name) = (found.id, found.name.clone());
        let receivers: Vec<ConnectionId> = self.connections_that_know(found).collect();
        self.channels
            .delete(&mut self.store, member.account, id)
            .map_err(|_| ErrorCode::ServerError)?
            .map_err(owner_error)?;
        Ok(receivers
            .into_iter()
            .map(|to| Outgoing {
                to,
                message: ServerMessage::ChannelDelete {
                    reference: request.reference.filter(|_| to == request.from),
                    channel: name.clone(),
                    time: request.now,
                },
            })
            .collect())
    }
}

fn owner_error(error: OwnerError) -> ErrorCode {
    match error {
        OwnerError::NoSuchChannel => ErrorCode::NoSuchChannel,
        OwnerError::NotPermitted => ErrorCode::NotPermitted,
    }
}

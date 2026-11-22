use post_proto::{ErrorCode, ServerMessage, check_topic};
use post_rules::OwnerError;

use super::{Answer, Hub, Request, present};

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
            .map_err(|error| match error {
                OwnerError::NoSuchChannel => ErrorCode::NoSuchChannel,
                OwnerError::NotPermitted => ErrorCode::NotPermitted,
            })?;
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
}

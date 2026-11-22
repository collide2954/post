use std::collections::BTreeMap;
use std::num::NonZeroU32;

use post_proto::{
    ChannelKind, ChannelName, ClientMessage, Code, ErrorCode, Invitation, ListedChannel, Name,
    ServerMessage, TextError, Timestamp, check_text,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Message {
        time: Timestamp,
        from: Name,
        text: String,
    },
    Joined {
        time: Timestamp,
        name: Name,
    },
    Left {
        time: Timestamp,
        name: Name,
    },
    Renamed {
        time: Timestamp,
        old: Name,
        new: Name,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    kind: ChannelKind,
    topic: String,
    owner: Name,
    created: Timestamp,
    entries: Vec<Entry>,
}

#[derive(Debug)]
pub struct Session {
    name: Name,
    channels: BTreeMap<ChannelName, Channel>,
    is_ready: bool,
    listed: Option<Vec<ListedChannel>>,
    listing: Option<(u32, Vec<ListedChannel>)>,
    problem: Option<ErrorCode>,
    invitations: Option<Vec<Invitation>>,
    next_reference: u32,
}

impl Channel {
    #[must_use]
    pub fn kind(&self) -> ChannelKind {
        self.kind
    }

    #[must_use]
    pub fn topic(&self) -> &str {
        &self.topic
    }

    #[must_use]
    pub fn owner(&self) -> &Name {
        &self.owner
    }

    #[must_use]
    pub fn created(&self) -> Timestamp {
        self.created
    }

    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn mentions(&self, name: &Name) -> bool {
        self.owner == *name || self.entries.iter().any(|entry| entry.mentions(name))
    }

    fn renamed(&mut self, old: &Name, new: &Name) {
        if self.owner == *old {
            self.owner = new.clone();
        }
        rename_in(&mut self.entries, old, new);
    }
}

impl Entry {
    fn mentions(&self, who: &Name) -> bool {
        match self {
            Self::Message { from, .. } => from == who,
            Self::Joined { name, .. } | Self::Left { name, .. } => name == who,
            Self::Renamed { .. } => false,
        }
    }
}

impl Session {
    #[must_use]
    pub fn new(name: Name) -> Self {
        Self::with_next_reference(name, 1)
    }

    fn with_next_reference(name: Name, next_reference: u32) -> Self {
        Self {
            name,
            channels: BTreeMap::new(),
            is_ready: false,
            listed: None,
            listing: None,
            problem: None,
            invitations: None,
            next_reference,
        }
    }

    #[must_use]
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn channels(&self) -> impl Iterator<Item = (&ChannelName, &Channel)> {
        self.channels.iter()
    }

    #[must_use]
    pub fn channel(&self, name: &ChannelName) -> Option<&Channel> {
        self.channels.get(name)
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.is_ready
    }

    #[must_use]
    pub fn listed(&self) -> Option<&[ListedChannel]> {
        self.listed.as_deref()
    }

    #[must_use]
    pub fn problem(&self) -> Option<ErrorCode> {
        self.problem
    }

    #[must_use]
    pub fn invitations(&self) -> Option<&[Invitation]> {
        self.invitations.as_deref()
    }

    pub fn dismiss_problem(&mut self) {
        self.problem = None;
    }

    pub fn compose(
        &mut self,
        channel: &ChannelName,
        text: &str,
    ) -> Result<ClientMessage, TextError> {
        check_text(text)?;
        Ok(ClientMessage::Send {
            reference: Some(self.request()),
            channel: channel.as_str().to_owned(),
            text: text.to_owned(),
        })
    }

    pub fn create_channel(&mut self, name: &ChannelName, kind: ChannelKind) -> ClientMessage {
        ClientMessage::ChannelCreate {
            reference: Some(self.request()),
            channel: name.as_str().to_owned(),
            kind,
        }
    }

    pub fn join(&mut self, name: &ChannelName) -> ClientMessage {
        ClientMessage::Join {
            reference: Some(self.request()),
            channel: name.as_str().to_owned(),
        }
    }

    pub fn leave(&mut self, name: &ChannelName) -> ClientMessage {
        ClientMessage::Leave {
            reference: Some(self.request()),
            channel: name.as_str().to_owned(),
        }
    }

    pub fn ask_for_channels(&mut self) -> ClientMessage {
        let reference = self.request();
        self.listing = Some((reference, Vec::new()));
        ClientMessage::Channels {
            reference: Some(reference),
        }
    }

    pub fn invite(
        &mut self,
        lifetime: Option<NonZeroU32>,
        uses: Option<NonZeroU32>,
    ) -> ClientMessage {
        ClientMessage::Invite {
            reference: Some(self.request()),
            lifetime,
            uses,
        }
    }

    pub fn ask_for_invitations(&mut self) -> ClientMessage {
        ClientMessage::Invitations {
            reference: Some(self.request()),
        }
    }

    pub fn revoke(&mut self, code: Code) -> ClientMessage {
        ClientMessage::Revoke {
            reference: Some(self.request()),
            code,
        }
    }

    pub fn rename(&mut self, name: &Name) -> ClientMessage {
        ClientMessage::Rename {
            reference: Some(self.request()),
            name: name.as_str().to_owned(),
        }
    }

    fn request(&mut self) -> u32 {
        let reference = self.next_reference;
        self.next_reference = reference.wrapping_add(1);
        self.problem = None;
        reference
    }

    pub fn apply(&mut self, message: ServerMessage) {
        match message {
            ServerMessage::Message {
                channel,
                from,
                time,
                text,
                ..
            } => self.record(&channel, Entry::Message { time, from, text }),
            ServerMessage::Join {
                channel,
                name,
                time,
            } => self.record(&channel, Entry::Joined { time, name }),
            ServerMessage::Leave {
                channel,
                name,
                time,
                ..
            } => self.left(&channel, name, time),
            ServerMessage::Channel {
                channel,
                kind,
                topic,
                owner,
                created,
                ..
            } => {
                let state = self.channels.entry(channel).or_insert_with(|| Channel {
                    kind,
                    topic: String::new(),
                    owner: owner.clone(),
                    created,
                    entries: Vec::new(),
                });
                state.kind = kind;
                state.topic = topic;
                state.owner = owner;
                state.created = created;
            }
            ServerMessage::Ready {} => self.is_ready = true,
            ServerMessage::Channels {
                reference,
                list,
                more,
            } => self.listed_part(reference, list, more),
            ServerMessage::Invitations { list, .. } => self.invitations = Some(list),
            ServerMessage::Rename { old, new, time, .. } => self.renamed(&old, &new, time),
            ServerMessage::Error { code, .. } => self.problem = Some(code),
            ServerMessage::Welcome { .. }
            | ServerMessage::Challenge { .. }
            | ServerMessage::ChannelTopic { .. }
            | ServerMessage::ChannelRename { .. }
            | ServerMessage::ChannelGive { .. }
            | ServerMessage::ChannelDelete { .. }
            | ServerMessage::ChannelInvite { .. }
            | ServerMessage::ChannelCancel { .. }
            | ServerMessage::ChannelRefuse { .. } => {}
        }
    }

    fn listed_part(&mut self, reference: Option<u32>, list: Vec<ListedChannel>, more: bool) {
        let Some((wanted, listing)) = &mut self.listing else {
            return;
        };
        if reference != Some(*wanted) {
            return;
        }
        listing.extend(list);
        if !more {
            self.listed = self.listing.take().map(|(_, complete)| complete);
        }
    }

    fn record(&mut self, channel: &ChannelName, entry: Entry) {
        if let Some(state) = self.channels.get_mut(channel) {
            state.entries.push(entry);
        }
    }

    fn left(&mut self, channel: &ChannelName, name: Name, time: Timestamp) {
        if name == self.name {
            self.channels.remove(channel);
        } else {
            self.record(channel, Entry::Left { time, name });
        }
    }

    fn renamed(&mut self, old: &Name, new: &Name, time: Timestamp) {
        if self.name == *old {
            self.name = new.clone();
        }
        for channel in self.channels.values_mut() {
            if channel.mentions(old) {
                channel.renamed(old, new);
                channel.entries.push(Entry::Renamed {
                    time,
                    old: old.clone(),
                    new: new.clone(),
                });
            }
        }
    }
}

fn rename_in(entries: &mut [Entry], old: &Name, new: &Name) {
    for entry in entries {
        let name = match entry {
            Entry::Message { from, .. } => from,
            Entry::Joined { name, .. } | Entry::Left { name, .. } => name,
            Entry::Renamed { .. } => continue,
        };
        if name == old {
            *name = new.clone();
        }
    }
}

#[cfg(test)]
mod tests;

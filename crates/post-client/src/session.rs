use std::num::NonZeroU32;

use post_proto::{
    ClientMessage, Code, ErrorCode, Invitation, LOBBY, Name, ServerMessage, TextError, Timestamp,
    check_text,
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

#[derive(Debug)]
pub struct Session {
    name: Name,
    entries: Vec<Entry>,
    problem: Option<ErrorCode>,
    invitations: Option<Vec<Invitation>>,
    next_reference: u32,
}

impl Session {
    #[must_use]
    pub fn new(name: Name) -> Self {
        Self::with_next_reference(name, 1)
    }

    fn with_next_reference(name: Name, next_reference: u32) -> Self {
        Self {
            name,
            entries: Vec::new(),
            problem: None,
            invitations: None,
            next_reference,
        }
    }

    #[must_use]
    pub fn name(&self) -> &Name {
        &self.name
    }

    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
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

    pub fn compose(&mut self, text: &str) -> Result<ClientMessage, TextError> {
        check_text(text)?;
        Ok(ClientMessage::Send {
            reference: Some(self.request()),
            channel: LOBBY.to_owned(),
            text: text.to_owned(),
        })
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

    fn renamed(&mut self, old: Name, new: Name, time: Timestamp) {
        if self.name == old {
            self.name = new.clone();
        }
        for entry in &mut self.entries {
            let name = match entry {
                Entry::Message { from, .. } => from,
                Entry::Joined { name, .. } | Entry::Left { name, .. } => name,
                Entry::Renamed { .. } => continue,
            };
            if *name == old {
                *name = new.clone();
            }
        }
        self.entries.push(Entry::Renamed { time, old, new });
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
            } if channel == LOBBY => self.entries.push(Entry::Message { time, from, text }),
            ServerMessage::Join {
                channel,
                name,
                time,
            } if channel == LOBBY => self.entries.push(Entry::Joined { time, name }),
            ServerMessage::Leave {
                channel,
                name,
                time,
                ..
            } if channel == LOBBY => self.entries.push(Entry::Left { time, name }),
            ServerMessage::Invitations { list, .. } => self.invitations = Some(list),
            ServerMessage::Rename { old, new, time, .. } => self.renamed(old, new, time),
            ServerMessage::Error { code, .. } => self.problem = Some(code),
            ServerMessage::Message { .. }
            | ServerMessage::Join { .. }
            | ServerMessage::Leave { .. }
            | ServerMessage::Welcome { .. }
            | ServerMessage::Channel { .. }
            | ServerMessage::Channels { .. }
            | ServerMessage::Ready {}
            | ServerMessage::Challenge { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests;

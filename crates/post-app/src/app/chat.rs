use std::collections::BTreeMap;
use std::num::NonZeroU32;

use post_client::{Channel, Entry, KnownServer, Link, Security, Session};
use post_proto::{
    ChannelKind, ChannelName, ChannelNameError, ClientMessage, Code, ErrorCode, Name, NameError,
    Role, ServerMessage, TextError,
};

use super::servers::{Form, Target};
use super::{Effect, error_text};
use crate::i18n::Text;

#[derive(Debug, Clone)]
pub enum Message {
    DraftChanged(String),
    Send,
    Leave,
    Open(Panel),
    Close,
    LifetimeSelected(Lifetime),
    UsesSelected(Uses),
    Invite,
    Revoke(Code),
    Copy(Code),
    NameChanged(String),
    Rename,
    Select(ChannelName),
    ChannelNameChanged(String),
    KindSelected(ChannelKind),
    MakeChannel,
    Join(ChannelName),
    LeaveChannel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Invitations,
    Account,
    Menu,
    Channels,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lifetime {
    Day,
    #[default]
    Week,
    Month,
    Forever,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Uses {
    One,
    Five,
    TwentyFive,
    #[default]
    NoLimit,
}

#[derive(Debug)]
pub struct Chat {
    form: Form,
    target: Target,
    link: Link,
    session: Session,
    place: Option<ChannelName>,
    drafts: BTreeMap<ChannelName, String>,
    notice: Option<Notice>,
    panel: Option<Open>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Open {
    Invitations(Invitations),
    Account(Account),
    Menu,
    Channels(Channels),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Invitations {
    pub lifetime: Lifetime,
    pub uses: Uses,
    pub copied: Option<Code>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Account {
    pub name: String,
    pub problem: Option<NameError>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Channels {
    pub name: String,
    pub kind: ChannelKind,
    pub problem: Option<ChannelNameError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Notice {
    TooLong,
    NotSent,
    RequestNotSent,
    ServerNotSaved,
}

impl Channels {
    #[must_use]
    pub fn problem_text(&self) -> Option<Text> {
        self.problem.map(|problem| match problem {
            ChannelNameError::InvalidCharacter => Text::ProblemChannelNameCharacters,
            ChannelNameError::TooShort => Text::ProblemChannelNameShort,
            ChannelNameError::TooLong => Text::ProblemChannelNameLong,
        })
    }
}

impl Lifetime {
    pub const ALL: [Self; 4] = [Self::Day, Self::Week, Self::Month, Self::Forever];

    #[must_use]
    pub fn seconds(self) -> Option<NonZeroU32> {
        const DAY: u32 = 24 * 60 * 60;
        NonZeroU32::new(match self {
            Self::Day => DAY,
            Self::Week => 7 * DAY,
            Self::Month => 30 * DAY,
            Self::Forever => 0,
        })
    }

    #[must_use]
    pub fn text(self) -> Text {
        match self {
            Self::Day => Text::InvitationsLifetimeDay,
            Self::Week => Text::InvitationsLifetimeWeek,
            Self::Month => Text::InvitationsLifetimeMonth,
            Self::Forever => Text::InvitationsLifetimeForever,
        }
    }
}

impl Uses {
    pub const ALL: [Self; 4] = [Self::One, Self::Five, Self::TwentyFive, Self::NoLimit];

    #[must_use]
    pub fn count(self) -> Option<NonZeroU32> {
        NonZeroU32::new(match self {
            Self::One => 1,
            Self::Five => 5,
            Self::TwentyFive => 25,
            Self::NoLimit => 0,
        })
    }
}

impl Chat {
    #[must_use]
    pub fn new(form: Form, target: Target, link: Link) -> Self {
        Self {
            form,
            target,
            session: Session::new(link.name().clone()),
            link,
            place: None,
            drafts: BTreeMap::new(),
            notice: None,
            panel: None,
        }
    }

    #[must_use]
    pub fn target(&self) -> &Target {
        &self.target
    }

    #[must_use]
    pub fn session(&self) -> &Session {
        &self.session
    }

    #[must_use]
    pub fn place(&self) -> Option<&ChannelName> {
        self.place.as_ref()
    }

    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        self.place
            .as_ref()
            .and_then(|place| self.session.channel(place))
            .map_or(&[], Channel::entries)
    }

    #[must_use]
    pub fn draft(&self) -> &str {
        self.place
            .as_ref()
            .and_then(|place| self.drafts.get(place))
            .map_or("", String::as_str)
    }

    #[must_use]
    pub fn panel(&self) -> Option<&Open> {
        self.panel.as_ref()
    }

    #[must_use]
    pub fn is_owner(&self) -> bool {
        self.link.role() == Role::Owner
    }

    #[must_use]
    pub fn notice(&self) -> Option<Text> {
        match self.notice {
            Some(Notice::TooLong) => Some(Text::ErrorTextTooLong),
            Some(Notice::NotSent) => Some(Text::ChatNotSent),
            Some(Notice::RequestNotSent) => Some(Text::ChatRequestNotSent),
            Some(Notice::ServerNotSaved) => Some(Text::ChatServerNotSaved),
            None => self.session.problem().map(|code| match code {
                ErrorCode::NameTaken if matches!(self.panel, Some(Open::Channels(_))) => {
                    Text::ErrorChannelNameTaken
                }
                code => error_text(code),
            }),
        }
    }

    #[must_use]
    pub fn server(&self) -> KnownServer {
        let destination = &self.target.destination;
        KnownServer {
            address: destination.address().to_string(),
            key: *self.link.server(),
            name: self.session.name().clone(),
            tls: destination.security() == Security::Tls,
            certificate: self
                .link
                .certificate()
                .map(|fingerprint| fingerprint.to_string()),
        }
    }

    pub fn update(&mut self, message: Message) -> Effect {
        match message {
            Message::DraftChanged(draft) => {
                if let Some(place) = &self.place {
                    self.drafts.insert(place.clone(), draft);
                }
                self.notice = None;
            }
            Message::Send => self.send(),
            Message::Leave => {}
            Message::Open(Panel::Invitations) => {
                self.panel = Some(Open::Invitations(Invitations::default()));
                let request = self.session.ask_for_invitations();
                self.request(request);
            }
            Message::Open(Panel::Account) => {
                self.forget_the_problem();
                self.panel = Some(Open::Account(Account {
                    name: self.session.name().as_str().to_owned(),
                    problem: None,
                }));
                return Effect::FocusPanel;
            }
            Message::Open(Panel::Menu) => self.panel = Some(Open::Menu),
            Message::Open(Panel::Channels) => {
                self.forget_the_problem();
                return self.open_channels();
            }
            Message::Close => return self.close(),
            Message::LifetimeSelected(lifetime) => {
                if let Some(Open::Invitations(invitations)) = &mut self.panel {
                    invitations.lifetime = lifetime;
                }
            }
            Message::UsesSelected(uses) => {
                if let Some(Open::Invitations(invitations)) = &mut self.panel {
                    invitations.uses = uses;
                }
            }
            Message::Invite => {
                if let Some(Open::Invitations(invitations)) = &mut self.panel {
                    invitations.copied = None;
                    let (lifetime, uses) = (invitations.lifetime, invitations.uses);
                    let request = self.session.invite(lifetime.seconds(), uses.count());
                    self.request(request);
                }
            }
            Message::Revoke(code) => {
                if matches!(self.panel, Some(Open::Invitations(_))) {
                    let request = self.session.revoke(code);
                    self.request(request);
                }
            }
            Message::Copy(code) => return self.copy(code),
            Message::NameChanged(name) => {
                if let Some(Open::Account(account)) = &mut self.panel {
                    account.name = name;
                    account.problem = None;
                }
            }
            Message::Rename => return self.rename(),
            Message::Select(channel) => return self.select(channel),
            Message::ChannelNameChanged(name) => {
                if let Some(Open::Channels(channels)) = &mut self.panel {
                    channels.name = name;
                    channels.problem = None;
                }
            }
            Message::KindSelected(kind) => {
                if let Some(Open::Channels(channels)) = &mut self.panel {
                    channels.kind = kind;
                }
            }
            Message::MakeChannel => self.make_channel(),
            Message::Join(name) => {
                let request = self.session.join(&name);
                self.request(request);
            }
            Message::LeaveChannel => {
                if let Some(place) = &self.place {
                    let request = self.session.leave(place);
                    self.request(request);
                }
            }
        }
        Effect::None
    }

    pub fn receive(&mut self, message: ServerMessage) -> Effect {
        let own_name_changes = matches!(
            &message,
            ServerMessage::Rename { old, .. } if old == self.session.name()
        );
        let answer = match &message {
            ServerMessage::Channel {
                reference: Some(_),
                channel,
                ..
            } => Some(channel.clone()),
            _ => None,
        };
        let was_ready = self.session.is_ready();
        self.session.apply(message);
        let moved = self.keep_a_place();
        if let Some(channel) = answer {
            return self.select(channel);
        }
        if !was_ready && self.session.is_ready() && self.place.is_none() {
            return self.open_channels();
        }
        if own_name_changes {
            if matches!(self.panel, Some(Open::Account(_))) {
                self.panel = None;
            }
            return Effect::Remember(self.server());
        }
        if moved && self.panel.is_none() {
            return Effect::FocusDraft;
        }
        Effect::None
    }

    #[must_use]
    pub fn send_line(&self, line: String) -> bool {
        self.link.send_line(line)
    }

    pub fn warn_that_the_server_is_not_saved(&mut self) {
        self.notice = Some(Notice::ServerNotSaved);
    }

    #[must_use]
    pub fn into_form(self) -> Form {
        self.form
    }

    fn keep_a_place(&mut self) -> bool {
        if let Some(place) = &self.place {
            if self.session.channel(place).is_some() {
                return false;
            }
            self.drafts.remove(place);
        }
        self.place = if self.session.is_ready() {
            self.session.channels().next().map(|(name, _)| name.clone())
        } else {
            None
        };
        self.place.is_some()
    }

    fn open_channels(&mut self) -> Effect {
        self.panel = Some(Open::Channels(Channels {
            name: String::new(),
            kind: ChannelKind::Public,
            problem: None,
        }));
        let request = self.session.ask_for_channels();
        if !self.link.send(request) {
            self.notice = Some(Notice::RequestNotSent);
        }
        Effect::FocusPanel
    }

    fn select(&mut self, channel: ChannelName) -> Effect {
        if self.session.channel(&channel).is_none() {
            return Effect::None;
        }
        self.place = Some(channel);
        self.panel = None;
        self.forget_the_problem();
        Effect::FocusDraft
    }

    fn send(&mut self) {
        let Some(place) = &self.place else {
            return;
        };
        let draft = self.drafts.get(place).map_or("", String::as_str);
        match self.session.compose(place, draft) {
            Ok(message) => {
                if self.link.send(message) {
                    self.drafts.remove(place);
                    self.notice = None;
                } else {
                    self.notice = Some(Notice::NotSent);
                }
            }
            Err(TextError::TooLong) => self.notice = Some(Notice::TooLong),
            Err(TextError::Empty) => {}
        }
    }

    fn make_channel(&mut self) {
        let Some(Open::Channels(channels)) = &mut self.panel else {
            return;
        };
        match ChannelName::parse(channels.name.trim()) {
            Ok(name) => {
                let request = self.session.create_channel(&name, channels.kind);
                self.request(request);
            }
            Err(problem) => channels.problem = Some(problem),
        }
    }

    fn request(&mut self, message: ClientMessage) {
        self.notice = if self.link.send(message) {
            None
        } else {
            Some(Notice::RequestNotSent)
        };
    }

    fn copy(&mut self, code: Code) -> Effect {
        let Some(Open::Invitations(invitations)) = &mut self.panel else {
            return Effect::None;
        };
        let link = self
            .session
            .invitations()
            .unwrap_or_default()
            .iter()
            .find(|invitation| invitation.code == code)
            .map(|invitation| invitation.link.clone());
        let Some(link) = link else {
            return Effect::None;
        };
        invitations.copied = Some(code);
        Effect::Copy(link)
    }

    fn rename(&mut self) -> Effect {
        let Some(Open::Account(account)) = &mut self.panel else {
            return Effect::None;
        };
        match Name::parse(account.name.trim()) {
            Ok(name) if name == *self.session.name() => return self.close(),
            Ok(name) => {
                let request = self.session.rename(&name);
                self.request(request);
            }
            Err(error) => account.problem = Some(error),
        }
        Effect::None
    }

    fn close(&mut self) -> Effect {
        if self.panel.take().is_none() {
            return Effect::None;
        }
        self.forget_the_problem();
        Effect::FocusDraft
    }

    fn forget_the_problem(&mut self) {
        self.notice = None;
        self.session.dismiss_problem();
    }
}

#[cfg(test)]
mod tests;

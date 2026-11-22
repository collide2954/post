use std::num::NonZeroU32;

use post_client::{KnownServer, Link, Security, Session};
use post_proto::{ClientMessage, Code, Name, NameError, Role, ServerMessage, TextError};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Invitations,
    Account,
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
    draft: String,
    notice: Option<Notice>,
    panel: Option<Open>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Open {
    Invitations(Invitations),
    Account(Account),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Notice {
    TooLong,
    NotSent,
    RequestNotSent,
    ServerNotSaved,
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
            draft: String::new(),
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
    pub fn draft(&self) -> &str {
        &self.draft
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
            None => self.session.problem().map(error_text),
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
                self.draft = draft;
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
        }
        Effect::None
    }

    pub fn receive(&mut self, message: ServerMessage) -> Effect {
        let own_name_changes = matches!(
            &message,
            ServerMessage::Rename { old, .. } if old == self.session.name()
        );
        self.session.apply(message);
        if !own_name_changes {
            return Effect::None;
        }
        if matches!(self.panel, Some(Open::Account(_))) {
            self.panel = None;
        }
        Effect::Remember(self.server())
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

    fn send(&mut self) {
        match self.session.compose(&self.draft) {
            Ok(message) => {
                if self.link.send(message) {
                    self.draft.clear();
                    self.notice = None;
                } else {
                    self.notice = Some(Notice::NotSent);
                }
            }
            Err(TextError::TooLong) => self.notice = Some(Notice::TooLong),
            Err(TextError::Empty) => {}
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

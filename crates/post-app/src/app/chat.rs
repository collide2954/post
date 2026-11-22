use post_client::{Link, Session};
use post_proto::{ServerMessage, TextError};

use super::error_text;
use super::servers::{Form, Target};
use crate::i18n::Text;

#[derive(Debug, Clone)]
pub enum Message {
    DraftChanged(String),
    Send,
    Leave,
}

#[derive(Debug)]
pub struct Chat {
    form: Form,
    target: Target,
    link: Link,
    session: Session,
    draft: String,
    notice: Option<Notice>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Notice {
    TooLong,
    NotSent,
    ServerNotSaved,
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
    pub fn notice(&self) -> Option<Text> {
        match self.notice {
            Some(Notice::TooLong) => Some(Text::ErrorTextTooLong),
            Some(Notice::NotSent) => Some(Text::ChatNotSent),
            Some(Notice::ServerNotSaved) => Some(Text::ChatServerNotSaved),
            None => self.session.problem().map(error_text),
        }
    }

    pub fn edit(&mut self, draft: String) {
        self.draft = draft;
        self.notice = None;
    }

    pub fn send(&mut self) {
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

    pub fn receive(&mut self, message: ServerMessage) {
        self.session.apply(message);
    }

    pub fn warn_that_the_server_is_not_saved(&mut self) {
        self.notice = Some(Notice::ServerNotSaved);
    }

    #[must_use]
    pub fn into_form(self) -> Form {
        self.form
    }
}

#[cfg(test)]
mod tests {
    use post_client::{Destination, Entry, Link, Trust};
    use post_proto::{ClientMessage, ErrorCode, Key, Name, Role, ServerMessage, Timestamp};

    use super::Chat;
    use crate::app::servers::{Form, Target};
    use crate::i18n::Text;

    type Outbox = Box<dyn FnMut() -> Option<ClientMessage>>;

    fn name(text: &str) -> Name {
        Name::parse(text).expect(text)
    }

    fn chat(capacity: usize) -> (Chat, Outbox) {
        let form = Form {
            address: "localhost".to_owned(),
            name: "ana".to_owned(),
            problem: None,
        };
        let target = Target {
            attempt: 3,
            destination: Destination::parse("localhost").expect("valid"),
            name: Some(name("ana")),
            trust: Trust::default(),
        };
        let (link, mut messages) = Link::channel(
            name("ana-2"),
            Role::Member,
            Key::from_bytes([1; 32]),
            capacity,
        );
        let chat = Chat::new(form, target, link);
        (chat, Box::new(move || messages.try_recv().ok()))
    }

    #[test]
    fn starts_with_the_name_that_the_server_gave() {
        let (chat, _outbox) = chat(4);

        assert_eq!(chat.session().name(), &name("ana-2"));
        assert_eq!(chat.draft(), "");
        assert_eq!(chat.notice(), None);
        assert_eq!(chat.target().attempt, 3);
    }

    #[test]
    fn records_what_the_server_sends() {
        let (mut chat, _outbox) = chat(4);
        let time = Timestamp::parse("2026-11-21T14:02:00.000Z").expect("valid");

        chat.receive(ServerMessage::Join {
            channel: "lobby".to_owned(),
            name: name("ben"),
            time,
        });

        assert_eq!(
            chat.session().entries(),
            [Entry::Joined {
                time,
                name: name("ben"),
            }]
        );
    }

    #[test]
    fn sends_the_draft_and_makes_the_field_empty() {
        let (mut chat, mut outbox) = chat(4);
        chat.edit("Hello".to_owned());
        assert_eq!(chat.draft(), "Hello");

        chat.send();

        assert_eq!(
            outbox(),
            Some(ClientMessage::Send {
                reference: Some(1),
                channel: "lobby".to_owned(),
                text: "Hello".to_owned(),
            })
        );
        assert_eq!(outbox(), None);
        assert_eq!(chat.draft(), "");
        assert_eq!(chat.notice(), None);
    }

    #[test]
    fn does_not_send_an_empty_draft() {
        let (mut chat, mut outbox) = chat(4);
        chat.edit("   ".to_owned());

        chat.send();

        assert_eq!(outbox(), None);
        assert_eq!(chat.draft(), "   ");
        assert_eq!(chat.notice(), None);
    }

    #[test]
    fn tells_the_user_that_a_draft_is_too_long() {
        let (mut chat, mut outbox) = chat(4);
        let long = "x".repeat(4001);
        chat.edit(long.clone());

        chat.send();

        assert_eq!(outbox(), None);
        assert_eq!(chat.draft(), long);
        assert_eq!(chat.notice(), Some(Text::ErrorTextTooLong));

        chat.edit("short".to_owned());
        assert_eq!(chat.notice(), None);
    }

    #[test]
    fn keeps_the_draft_when_the_connection_cannot_take_it() {
        let (mut chat, mut outbox) = chat(1);
        chat.edit("one".to_owned());
        chat.send();
        chat.edit("two".to_owned());

        chat.send();

        assert_eq!(chat.draft(), "two");
        assert_eq!(chat.notice(), Some(Text::ChatNotSent));
        assert!(outbox().is_some());
        assert_eq!(outbox(), None);
    }

    #[test]
    fn shows_an_error_from_the_server_until_the_user_sends_again() {
        let (mut chat, _outbox) = chat(4);

        chat.receive(ServerMessage::error(Some(1), ErrorCode::NoSuchChannel));
        assert_eq!(chat.notice(), Some(Text::ErrorNoSuchChannel));

        chat.edit("Hello".to_owned());
        assert_eq!(chat.notice(), Some(Text::ErrorNoSuchChannel));
        chat.send();
        assert_eq!(chat.notice(), None);
    }

    #[test]
    fn tells_the_user_that_the_profile_did_not_save_the_server() {
        let (mut chat, _outbox) = chat(4);

        chat.warn_that_the_server_is_not_saved();
        assert_eq!(chat.notice(), Some(Text::ChatServerNotSaved));

        chat.edit("Hello".to_owned());
        assert_eq!(chat.notice(), None);
    }

    #[test]
    fn gives_the_form_back_when_the_user_leaves() {
        let (chat, _outbox) = chat(4);

        let form = chat.into_form();

        assert_eq!(form.address, "localhost");
        assert_eq!(form.name, "ana");
    }
}

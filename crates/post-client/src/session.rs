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
            } if channel == LOBBY => self.entries.push(Entry::Left { time, name }),
            ServerMessage::Invitations { list, .. } => self.invitations = Some(list),
            ServerMessage::Error { code, .. } => self.problem = Some(code),
            ServerMessage::Message { .. }
            | ServerMessage::Join { .. }
            | ServerMessage::Leave { .. }
            | ServerMessage::Welcome { .. }
            | ServerMessage::Challenge { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use post_proto::{
        ClientMessage, Code, ErrorCode, Invitation, Name, Role, ServerMessage, TextError, Timestamp,
    };

    use super::{Entry, Session};

    fn name(text: &str) -> Name {
        Name::parse(text).expect(text)
    }

    fn time(second: u8) -> Timestamp {
        Timestamp::parse(&format!("2026-11-21T14:02:{second:02}.000Z")).expect("valid")
    }

    fn join(channel: &str, who: &str, second: u8) -> ServerMessage {
        ServerMessage::Join {
            channel: channel.to_owned(),
            name: name(who),
            time: time(second),
        }
    }

    fn leave(channel: &str, who: &str, second: u8) -> ServerMessage {
        ServerMessage::Leave {
            channel: channel.to_owned(),
            name: name(who),
            time: time(second),
        }
    }

    fn message(channel: &str, from: &str, second: u8, text: &str) -> ServerMessage {
        ServerMessage::Message {
            reference: None,
            channel: channel.to_owned(),
            from: name(from),
            time: time(second),
            text: text.to_owned(),
        }
    }

    #[test]
    fn starts_with_a_name_and_an_empty_timeline() {
        let session = Session::new(name("ana"));

        assert_eq!(session.name(), &name("ana"));
        assert_eq!(session.entries(), []);
        assert_eq!(session.problem(), None);
        assert_eq!(session.invitations(), None);
    }

    #[test]
    fn keeps_the_newest_list_of_invitations_from_the_server() {
        let mut session = Session::new(name("ana"));
        let invitation = |text: &str, used| Invitation {
            code: Code::parse(text).expect(text),
            link: format!("https://chat.example.org/i/{text}"),
            created: time(1),
            end: None,
            uses: None,
            used,
        };

        session.apply(ServerMessage::Invitations {
            reference: Some(1),
            list: vec![invitation("7Hq2kdX9mPq4", 0)],
        });
        assert_eq!(
            session.invitations(),
            Some([invitation("7Hq2kdX9mPq4", 0)].as_slice())
        );

        let newer = vec![invitation("k3Wm8sTz2LpQ", 0), invitation("7Hq2kdX9mPq4", 1)];
        session.apply(ServerMessage::Invitations {
            reference: None,
            list: newer.clone(),
        });
        assert_eq!(session.invitations(), Some(newer.as_slice()));

        session.apply(ServerMessage::Invitations {
            reference: None,
            list: Vec::new(),
        });
        assert_eq!(session.invitations(), Some([].as_slice()));
        assert_eq!(session.entries(), []);
    }

    #[test]
    fn makes_each_invitation_request_with_a_new_number() {
        let mut session = Session::new(name("ana"));
        let code = Code::parse("7Hq2kdX9mPq4").expect("valid");

        assert_eq!(
            session.invite(NonZeroU32::new(604_800), NonZeroU32::new(5)),
            ClientMessage::Invite {
                reference: Some(1),
                lifetime: NonZeroU32::new(604_800),
                uses: NonZeroU32::new(5),
            }
        );
        assert_eq!(
            session.ask_for_invitations(),
            ClientMessage::Invitations { reference: Some(2) }
        );
        assert_eq!(
            session.revoke(code.clone()),
            ClientMessage::Revoke {
                reference: Some(3),
                code,
            }
        );
    }

    #[test]
    fn forgets_the_error_when_the_user_makes_a_new_request() {
        let mut session = Session::new(name("ana"));

        session.apply(ServerMessage::error(Some(1), ErrorCode::NotPermitted));
        assert_eq!(session.problem(), Some(ErrorCode::NotPermitted));

        let _request = session.ask_for_invitations();
        assert_eq!(session.problem(), None);
    }

    #[test]
    fn records_what_occurs_in_the_lobby_in_sequence() {
        let mut session = Session::new(name("ana"));

        session.apply(join("lobby", "ben", 1));
        session.apply(message("lobby", "ben", 2, "Hi"));
        session.apply(leave("lobby", "ben", 3));

        assert_eq!(
            session.entries(),
            [
                Entry::Joined {
                    time: time(1),
                    name: name("ben"),
                },
                Entry::Message {
                    time: time(2),
                    from: name("ben"),
                    text: "Hi".to_owned(),
                },
                Entry::Left {
                    time: time(3),
                    name: name("ben"),
                },
            ]
        );
    }

    #[test]
    fn ignores_what_occurs_in_a_different_channel() {
        let mut session = Session::new(name("ana"));

        session.apply(join("garden", "ben", 1));
        session.apply(message("garden", "ben", 2, "Hi"));
        session.apply(leave("garden", "ben", 3));

        assert_eq!(session.entries(), []);
    }

    #[test]
    fn ignores_a_second_welcome_message() {
        let mut session = Session::new(name("ana"));

        session.apply(ServerMessage::Welcome {
            reference: None,
            name: name("ben"),
            role: Role::Member,
        });

        assert_eq!(session.name(), &name("ana"));
        assert_eq!(session.entries(), []);
        assert_eq!(session.problem(), None);
    }

    #[test]
    fn makes_a_send_message_with_a_new_number_each_time() {
        let mut session = Session::new(name("ana"));

        assert_eq!(
            session.compose("Hello"),
            Ok(ClientMessage::Send {
                reference: Some(1),
                channel: "lobby".to_owned(),
                text: "Hello".to_owned(),
            })
        );
        assert_eq!(
            session.compose("  spaces stay  "),
            Ok(ClientMessage::Send {
                reference: Some(2),
                channel: "lobby".to_owned(),
                text: "  spaces stay  ".to_owned(),
            })
        );
    }

    #[test]
    fn does_not_make_a_message_from_a_text_that_breaks_the_rules() {
        let mut session = Session::new(name("ana"));

        assert_eq!(session.compose("   "), Err(TextError::Empty));
        assert_eq!(session.compose(&"x".repeat(4001)), Err(TextError::TooLong));
        assert!(matches!(
            session.compose("Hello"),
            Ok(ClientMessage::Send {
                reference: Some(1),
                ..
            })
        ));
    }

    #[test]
    fn remembers_the_last_error_from_the_server() {
        let mut session = Session::new(name("ana"));

        session.apply(ServerMessage::error(Some(1), ErrorCode::TextTooLong));
        assert_eq!(session.problem(), Some(ErrorCode::TextTooLong));

        session.apply(ServerMessage::error(None, ErrorCode::BadMessage));
        assert_eq!(session.problem(), Some(ErrorCode::BadMessage));
        assert_eq!(session.entries(), []);
    }

    #[test]
    fn forgets_the_error_when_the_user_sends_a_new_text() {
        let mut session = Session::new(name("ana"));
        session.apply(ServerMessage::error(Some(1), ErrorCode::TextTooLong));

        assert_eq!(session.compose(" "), Err(TextError::Empty));
        assert_eq!(session.problem(), Some(ErrorCode::TextTooLong));

        assert!(session.compose("Hello").is_ok());
        assert_eq!(session.problem(), None);
    }

    #[test]
    fn starts_the_numbers_again_after_the_largest_number() {
        let mut session = Session::with_next_reference(name("ana"), u32::MAX);

        assert!(matches!(
            session.compose("one"),
            Ok(ClientMessage::Send {
                reference: Some(u32::MAX),
                ..
            })
        ));
        assert!(matches!(
            session.compose("two"),
            Ok(ClientMessage::Send {
                reference: Some(0),
                ..
            })
        ));
    }
}

use post_client::{Destination, DestinationError, Disconnect, Fingerprint, KnownServer, Trust};
use post_proto::{Name, NameError};

use super::{Effect, error_text};
use crate::i18n::Text;

#[derive(Debug, Clone)]
pub enum Message {
    AddressChanged(String),
    NameChanged(String),
    Connect,
    Open(usize),
    Forget(usize),
    Cancel,
    AcceptCertificate,
}

#[derive(Debug, Default)]
pub struct Servers {
    form: Form,
    connecting: Option<Target>,
    question: Option<Question>,
}

#[derive(Debug, Clone, Default)]
pub struct Form {
    pub address: String,
    pub name: String,
    pub problem: Option<Problem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Target {
    pub attempt: u64,
    pub destination: Destination,
    pub name: Option<Name>,
    pub trust: Trust,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub target: Target,
    pub fingerprint: Fingerprint,
    pub changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    Address(DestinationError),
    Name(NameError),
    Disconnected(Disconnect),
    NotSaved,
}

impl Servers {
    #[must_use]
    pub fn with(form: Form) -> Self {
        Self {
            form,
            connecting: None,
            question: None,
        }
    }

    #[must_use]
    pub fn opening(server: &KnownServer, attempts: &mut u64) -> Self {
        let mut servers = Self::default();
        servers.open(server, attempts);
        servers
    }

    #[must_use]
    pub fn form(&self) -> &Form {
        &self.form
    }

    #[must_use]
    pub fn connecting(&self) -> Option<&Target> {
        self.connecting.as_ref()
    }

    #[must_use]
    pub fn question(&self) -> Option<&Question> {
        self.question.as_ref()
    }

    pub fn update(
        &mut self,
        message: Message,
        known: &mut Vec<KnownServer>,
        attempts: &mut u64,
    ) -> Effect {
        match message {
            Message::AddressChanged(address) => self.edit(|form| form.address = address),
            Message::NameChanged(name) => self.edit(|form| form.name = name),
            Message::Connect => {
                if self.is_idle() {
                    self.connect(known, attempts);
                }
                Effect::None
            }
            Message::Open(index) => {
                if self.is_idle()
                    && let Some(server) = known.get(index)
                {
                    self.open(server, attempts);
                }
                Effect::None
            }
            Message::Forget(index) => {
                if !self.is_idle() || index >= known.len() {
                    return Effect::None;
                }
                self.form.problem = None;
                Effect::Forget(known.remove(index).address)
            }
            Message::Cancel => {
                if self.question.take().is_some() {
                    return Effect::FocusForm;
                }
                self.stop(None)
            }
            Message::AcceptCertificate => {
                self.accept(attempts);
                Effect::None
            }
        }
    }

    pub fn stop(&mut self, problem: Option<Problem>) -> Effect {
        let Some(target) = self.connecting.take() else {
            return Effect::None;
        };
        let (fingerprint, changed) = match problem {
            Some(Problem::Disconnected(Disconnect::UnknownCertificate(fingerprint))) => {
                (fingerprint, false)
            }
            Some(Problem::Disconnected(Disconnect::ChangedCertificate(fingerprint))) => {
                (fingerprint, true)
            }
            other => {
                self.form.problem = other;
                return Effect::FocusForm;
            }
        };
        self.question = Some(Question {
            target,
            fingerprint,
            changed,
        });
        Effect::None
    }

    pub fn warn_that_the_profile_is_not_saved(&mut self) {
        self.form.problem = Some(Problem::NotSaved);
    }

    pub fn connected(&mut self) -> Option<(Form, Target)> {
        let target = self.connecting.take()?;
        Some((std::mem::take(&mut self.form), target))
    }

    fn is_idle(&self) -> bool {
        self.connecting.is_none() && self.question.is_none()
    }

    fn edit(&mut self, edit: impl FnOnce(&mut Form)) -> Effect {
        if self.is_idle() {
            edit(&mut self.form);
            self.form.problem = None;
        }
        Effect::None
    }

    fn connect(&mut self, known: &[KnownServer], attempts: &mut u64) {
        let destination = match Destination::parse(&self.form.address) {
            Ok(destination) => destination,
            Err(error) => {
                self.form.problem = Some(Problem::Address(error));
                return;
            }
        };
        let name = match Some(self.form.name.trim())
            .filter(|name| !name.is_empty())
            .map(Name::parse)
            .transpose()
        {
            Ok(name) => name,
            Err(error) => {
                self.form.problem = Some(Problem::Name(error));
                return;
            }
        };
        let address = destination.address().to_string();
        let trust = known
            .iter()
            .find(|server| server.address == address)
            .map_or_else(Trust::default, trust_in);
        self.start(destination, name, trust, attempts);
    }

    fn open(&mut self, server: &KnownServer, attempts: &mut u64) {
        match server.destination() {
            Some(destination) => self.start(destination, None, trust_in(server), attempts),
            None => self.form.problem = Some(Problem::Address(DestinationError::Invalid)),
        }
    }

    fn accept(&mut self, attempts: &mut u64) {
        let Some(question) = self.question.take() else {
            return;
        };
        let Target {
            destination,
            name,
            mut trust,
            ..
        } = question.target;
        trust.certificate = Some(question.fingerprint);
        self.start(destination, name, trust, attempts);
    }

    fn start(
        &mut self,
        destination: Destination,
        name: Option<Name>,
        trust: Trust,
        attempts: &mut u64,
    ) {
        *attempts += 1;
        self.form.problem = None;
        self.connecting = Some(Target {
            attempt: *attempts,
            destination,
            name,
            trust,
        });
    }
}

fn trust_in(server: &KnownServer) -> Trust {
    Trust {
        key: Some(server.key),
        certificate: server.certificate.as_deref().and_then(Fingerprint::parse),
    }
}

impl Problem {
    #[must_use]
    pub fn text(self) -> Text {
        match self {
            Self::Address(DestinationError::Empty) => Text::ProblemAddressEmpty,
            Self::Address(DestinationError::Invalid) => Text::ProblemAddressInvalid,
            Self::Address(DestinationError::InvalidPort) => Text::ProblemAddressPort,
            Self::Address(DestinationError::InvalidLink) => Text::ProblemAddressLink,
            Self::Address(DestinationError::NotLocal) => Text::ProblemAddressNotLocal,
            Self::Name(NameError::InvalidCharacter) => Text::ProblemNameCharacters,
            Self::Name(NameError::TooShort) => Text::ProblemNameShort,
            Self::Name(NameError::TooLong) => Text::ProblemNameLong,
            Self::Disconnected(Disconnect::Unreachable) => Text::ProblemUnreachable,
            Self::Disconnected(Disconnect::Closed) => Text::ProblemClosed,
            Self::Disconnected(Disconnect::Protocol) => Text::ProblemProtocol,
            Self::Disconnected(Disconnect::FalseServer) => Text::ProblemFalseServer,
            Self::Disconnected(Disconnect::DifferentServer) => Text::ProblemDifferentServer,
            Self::Disconnected(Disconnect::MissingParts) => Text::ProblemMissingParts,
            Self::Disconnected(Disconnect::NameRequired) => Text::ProblemNameRequired,
            Self::Disconnected(Disconnect::InvitationRequired) => Text::ProblemInvitationRequired,
            Self::Disconnected(Disconnect::UnknownCertificate(_)) => Text::CertificateUnknown,
            Self::Disconnected(Disconnect::ChangedCertificate(_)) => Text::CertificateChanged,
            Self::Disconnected(Disconnect::Refused(code)) => error_text(code),
            Self::NotSaved => Text::ServersNotSaved,
        }
    }
}

#[cfg(test)]
mod tests {
    use post_client::{
        Destination, DestinationError, Disconnect, Fingerprint, KnownServer, Security, Trust,
    };
    use post_proto::{ErrorCode, Key, Name, NameError};

    use super::{Form, Message, Problem, Servers};
    use crate::app::Effect;
    use crate::i18n::Text;

    const LINK: &str = "http://localhost:7678/i/7Hq2kdX9mPq4";

    fn name(text: &str) -> Name {
        Name::parse(text).expect(text)
    }

    fn key(number: u8) -> Key {
        Key::from_bytes([number; 32])
    }

    fn fingerprint(number: u8) -> Fingerprint {
        Fingerprint::from_bytes([number; 32])
    }

    fn server(address: &str, number: u8, who: &str) -> KnownServer {
        KnownServer {
            address: address.to_owned(),
            key: key(number),
            name: name(who),
            tls: false,
            certificate: None,
        }
    }

    fn typed(servers: &mut Servers, address: &str, name: &str) {
        let mut attempts = 0;
        let mut known = Vec::new();
        let address = Message::AddressChanged(address.to_owned());
        servers.update(address, &mut known, &mut attempts);
        let name = Message::NameChanged(name.to_owned());
        servers.update(name, &mut known, &mut attempts);
    }

    fn connect(servers: &mut Servers, known: &[KnownServer]) -> u64 {
        let mut attempts = 0;
        let mut known = known.to_vec();
        assert_eq!(
            servers.update(Message::Connect, &mut known, &mut attempts),
            Effect::None
        );
        attempts
    }

    fn connecting_to(address: &str) -> Servers {
        let mut servers = Servers::default();
        typed(&mut servers, address, "ana");
        assert_eq!(connect(&mut servers, &[]), 1);
        servers
    }

    fn refused(reason: Disconnect) -> Problem {
        Problem::Disconnected(reason)
    }

    #[test]
    fn starts_with_an_empty_form() {
        let servers = Servers::default();

        assert_eq!(servers.form().address, "");
        assert_eq!(servers.form().name, "");
        assert_eq!(servers.form().problem, None);
        assert_eq!(servers.connecting(), None);
        assert_eq!(servers.question(), None);
    }

    #[test]
    fn keeps_what_the_user_types() {
        let mut servers = Servers::default();

        typed(&mut servers, "127.0.0.1:9000", "ana");

        assert_eq!(servers.form().address, "127.0.0.1:9000");
        assert_eq!(servers.form().name, "ana");
    }

    #[test]
    fn tells_the_user_about_an_address_that_it_cannot_use() {
        for (address, error) in [
            ("", DestinationError::Empty),
            ("local host", DestinationError::Invalid),
            ("localhost:0", DestinationError::InvalidPort),
            ("localhost:7678/i/short", DestinationError::InvalidLink),
            ("http://chat.example.org", DestinationError::NotLocal),
        ] {
            let mut servers = Servers::default();
            typed(&mut servers, address, "ana");

            assert_eq!(connect(&mut servers, &[]), 0, "{address}");

            assert_eq!(
                servers.form().problem,
                Some(Problem::Address(error)),
                "{address}"
            );
            assert_eq!(servers.connecting(), None);
        }
    }

    #[test]
    fn tells_the_user_about_a_name_that_breaks_the_rules() {
        for (who, error) in [
            ("a", NameError::TooShort),
            ("Ana", NameError::InvalidCharacter),
            ("abcdefghij0123456789x", NameError::TooLong),
        ] {
            let mut servers = Servers::default();
            typed(&mut servers, "localhost", who);

            assert_eq!(connect(&mut servers, &[]), 0, "{who}");

            assert_eq!(servers.form().problem, Some(Problem::Name(error)), "{who}");
            assert_eq!(servers.connecting(), None);
        }
    }

    #[test]
    fn removes_the_problem_when_the_user_types() {
        let mut servers = Servers::default();
        connect(&mut servers, &[]);
        assert!(servers.form().problem.is_some());
        typed(&mut servers, "l", "ana");
        assert_eq!(servers.form().problem, None);

        typed(&mut servers, "localhost", "a");
        connect(&mut servers, &[]);
        assert!(servers.form().problem.is_some());
        typed(&mut servers, "localhost", "an");
        assert_eq!(servers.form().problem, None);
    }

    #[test]
    fn starts_a_connection_with_an_invite_link_and_a_name() {
        let mut servers = Servers::default();
        typed(&mut servers, &format!(" {LINK} "), "  ana  ");
        let mut attempts = 4;

        servers.update(Message::Connect, &mut Vec::new(), &mut attempts);

        assert_eq!(attempts, 5);
        let target = servers.connecting().expect("a connection is in progress");
        assert_eq!(target.attempt, 5);
        assert_eq!(target.destination, Destination::parse(LINK).expect("valid"));
        assert_eq!(target.name, Some(name("ana")));
        assert_eq!(target.trust, Trust::default());
        assert_eq!(servers.form().address, format!(" {LINK} "));
        assert_eq!(servers.form().problem, None);
    }

    #[test]
    fn starts_a_connection_with_an_address_and_no_name() {
        for who in ["", "   "] {
            let mut servers = Servers::default();
            typed(&mut servers, " LOCALHOST:9000 ", who);

            assert_eq!(connect(&mut servers, &[]), 1);

            let target = servers.connecting().expect("a connection is in progress");
            assert_eq!(
                target.destination,
                Destination::parse("localhost:9000").expect("valid")
            );
            assert_eq!(target.name, None, "{who:?}");
        }
    }

    #[test]
    fn gives_the_key_of_a_known_server_to_the_connection() {
        let known = [
            server("chat.example.org:443", 1, "ana"),
            server("localhost:7678", 2, "ana-dev"),
        ];
        for (address, expected) in [
            ("localhost", Some(key(2))),
            ("http://localhost:7678/", Some(key(2))),
            ("chat.example.org", Some(key(1))),
            ("localhost:9000", None),
            ("127.0.0.1", None),
        ] {
            let mut servers = Servers::default();
            typed(&mut servers, address, "");

            connect(&mut servers, &known);

            let target = servers.connecting().expect("a connection is in progress");
            assert_eq!(target.trust.key, expected, "{address}");
            assert_eq!(target.trust.certificate, None);
        }
    }

    #[test]
    fn gives_the_accepted_certificate_of_a_known_server_to_the_connection() {
        let pinned = KnownServer {
            tls: true,
            certificate: Some(fingerprint(9).to_string()),
            ..server("home.example.org:443", 1, "ana")
        };
        let damaged = KnownServer {
            certificate: Some("not a fingerprint".to_owned()),
            ..server("localhost:7678", 2, "ana")
        };
        let known = [pinned, damaged];

        let mut servers = Servers::default();
        typed(&mut servers, "home.example.org", "");
        connect(&mut servers, &known);
        let target = servers.connecting().expect("a connection is in progress");
        assert_eq!(target.trust.key, Some(key(1)));
        assert_eq!(target.trust.certificate, Some(fingerprint(9)));

        let mut servers = Servers::default();
        typed(&mut servers, "localhost", "");
        connect(&mut servers, &known);
        let target = servers.connecting().expect("a connection is in progress");
        assert_eq!(target.trust.key, Some(key(2)));
        assert_eq!(target.trust.certificate, None);
    }

    #[test]
    fn connects_to_a_server_of_the_list() {
        let mut known = vec![
            server("localhost:7678", 1, "ana"),
            KnownServer {
                tls: true,
                certificate: Some(fingerprint(9).to_string()),
                ..server("localhost:8443", 2, "ana")
            },
        ];
        let mut servers = Servers::default();
        typed(&mut servers, "something-else.example.org", "typed-name");
        let mut attempts = 0;

        let effect = servers.update(Message::Open(1), &mut known, &mut attempts);

        assert_eq!(effect, Effect::None);
        let target = servers.connecting().expect("a connection is in progress");
        assert_eq!(target.attempt, 1);
        assert_eq!(target.destination.to_string(), "https://localhost:8443");
        assert_eq!(target.destination.security(), Security::Tls);
        assert_eq!(target.name, None);
        assert_eq!(target.trust.key, Some(key(2)));
        assert_eq!(target.trust.certificate, Some(fingerprint(9)));
        assert_eq!(known.len(), 2);
    }

    #[test]
    fn connects_to_the_first_server_at_the_start() {
        let mut attempts = 0;

        let servers = Servers::opening(&server("localhost:7678", 1, "ana"), &mut attempts);

        assert_eq!(attempts, 1);
        let target = servers.connecting().expect("a connection is in progress");
        assert_eq!(target.destination.to_string(), "localhost:7678");
        assert_eq!(target.destination.security(), Security::Plain);
        assert_eq!(target.trust.key, Some(key(1)));
    }

    #[test]
    fn does_not_connect_to_an_entry_that_is_not_in_the_list_or_not_valid() {
        let mut known = vec![server("not an address", 1, "ana")];
        let mut servers = Servers::default();
        let mut attempts = 0;

        servers.update(Message::Open(5), &mut known, &mut attempts);
        assert_eq!(servers.connecting(), None);
        assert_eq!(servers.form().problem, None);

        servers.update(Message::Open(0), &mut known, &mut attempts);
        assert_eq!(servers.connecting(), None);
        assert_eq!(
            servers.form().problem,
            Some(Problem::Address(DestinationError::Invalid))
        );
        assert_eq!(attempts, 0);
    }

    #[test]
    fn removes_a_server_from_the_list() {
        let mut known = vec![
            server("localhost:7678", 1, "ana"),
            server("chat.example.org:443", 2, "ana"),
        ];
        let mut servers = Servers::default();
        servers.warn_that_the_profile_is_not_saved();
        let mut attempts = 0;

        let effect = servers.update(Message::Forget(0), &mut known, &mut attempts);

        assert_eq!(effect, Effect::Forget("localhost:7678".to_owned()));
        assert_eq!(known, [server("chat.example.org:443", 2, "ana")]);
        assert_eq!(servers.form().problem, None);
        assert_eq!(
            servers.update(Message::Forget(7), &mut known, &mut attempts),
            Effect::None
        );
        assert_eq!(known.len(), 1);
    }

    #[test]
    fn does_not_change_while_it_connects() {
        let mut servers = connecting_to("localhost");
        let mut known = vec![server("localhost:7678", 1, "ana")];
        let mut attempts = 1;

        typed(&mut servers, "x", "x");
        for message in [
            Message::Connect,
            Message::Open(0),
            Message::Forget(0),
            Message::AcceptCertificate,
        ] {
            assert_eq!(
                servers.update(message, &mut known, &mut attempts),
                Effect::None
            );
        }

        assert_eq!(servers.form().address, "localhost");
        assert_eq!(servers.form().name, "ana");
        assert_eq!(servers.connecting().map(|target| target.attempt), Some(1));
        assert_eq!((known.len(), attempts), (1, 1));
    }

    #[test]
    fn stops_a_connection_when_the_user_asks() {
        let mut servers = Servers::default();
        let mut attempts = 0;
        let mut known = Vec::new();
        assert_eq!(
            servers.update(Message::Cancel, &mut known, &mut attempts),
            Effect::None
        );
        typed(&mut servers, "localhost", "ana");
        servers.update(Message::Connect, &mut known, &mut attempts);

        assert_eq!(
            servers.update(Message::Cancel, &mut known, &mut attempts),
            Effect::FocusForm
        );

        assert_eq!(servers.connecting(), None);
        assert_eq!(servers.form().problem, None);
        assert_eq!(servers.form().name, "ana");
    }

    #[test]
    fn shows_why_a_connection_stopped() {
        let mut servers = Servers::default();
        let problem = refused(Disconnect::Unreachable);
        assert_eq!(servers.stop(Some(problem)), Effect::None);
        assert_eq!(servers.form().problem, None);
        typed(&mut servers, "localhost", "");
        connect(&mut servers, &[]);

        assert_eq!(servers.stop(Some(problem)), Effect::FocusForm);

        assert_eq!(servers.form().problem, Some(problem));
        assert_eq!(servers.connecting(), None);
        assert_eq!(servers.question(), None);
    }

    #[test]
    fn asks_the_user_about_a_certificate_that_no_authority_signed() {
        let mut servers = connecting_to("https://localhost:8443");
        let target = servers.connecting().cloned().expect("a connection");

        let effect = servers.stop(Some(refused(Disconnect::UnknownCertificate(fingerprint(
            9,
        )))));

        assert_eq!(effect, Effect::None);
        assert_eq!(servers.connecting(), None);
        assert_eq!(servers.form().problem, None);
        let question = servers.question().expect("a question");
        assert_eq!(question.target, target);
        assert_eq!(question.fingerprint, fingerprint(9));
        assert!(!question.changed);
    }

    #[test]
    fn connects_again_with_the_certificate_that_the_user_accepted() {
        let mut servers = connecting_to("https://localhost:8443");
        servers.stop(Some(refused(Disconnect::UnknownCertificate(fingerprint(
            9,
        )))));
        let mut attempts = 1;

        let effect = servers.update(Message::AcceptCertificate, &mut Vec::new(), &mut attempts);

        assert_eq!(effect, Effect::None);
        assert_eq!(servers.question(), None);
        let target = servers.connecting().expect("a second connection");
        assert_eq!(target.attempt, 2);
        assert_eq!(target.destination.to_string(), "https://localhost:8443");
        assert_eq!(target.name, Some(name("ana")));
        assert_eq!(target.trust.certificate, Some(fingerprint(9)));
    }

    #[test]
    fn warns_the_user_about_a_certificate_that_changed() {
        let known = KnownServer {
            tls: true,
            certificate: Some(fingerprint(8).to_string()),
            ..server("localhost:8443", 3, "ana")
        };
        let mut attempts = 0;
        let mut servers = Servers::opening(&known, &mut attempts);

        servers.stop(Some(refused(Disconnect::ChangedCertificate(fingerprint(
            9,
        )))));

        let question = servers.question().expect("a question");
        assert!(question.changed);
        assert_eq!(question.fingerprint, fingerprint(9));
        servers.update(Message::AcceptCertificate, &mut vec![known], &mut attempts);
        let target = servers.connecting().expect("a second connection");
        assert_eq!(target.trust.key, Some(key(3)));
        assert_eq!(target.trust.certificate, Some(fingerprint(9)));
    }

    #[test]
    fn does_not_connect_when_the_user_refuses_the_certificate() {
        let mut servers = connecting_to("https://localhost:8443");
        servers.stop(Some(refused(Disconnect::UnknownCertificate(fingerprint(
            9,
        )))));
        let mut attempts = 1;
        let mut known = vec![server("localhost:7678", 1, "ana")];
        typed(&mut servers, "x", "x");
        servers.update(Message::Open(0), &mut known, &mut attempts);
        assert!(servers.question().is_some());
        assert_eq!(servers.connecting(), None);

        let effect = servers.update(Message::Cancel, &mut known, &mut attempts);

        assert_eq!(effect, Effect::FocusForm);
        assert_eq!(servers.question(), None);
        assert_eq!(servers.connecting(), None);
        assert_eq!(servers.form().address, "https://localhost:8443");
        assert_eq!(attempts, 1);
    }

    #[test]
    fn gives_its_form_and_target_when_the_connection_opens() {
        let mut servers = Servers::default();
        assert!(servers.connected().is_none());
        typed(&mut servers, "localhost", "ana");
        connect(&mut servers, &[]);

        let (form, target) = servers.connected().expect("a connection was in progress");

        assert_eq!(form.address, "localhost");
        assert_eq!(form.name, "ana");
        assert_eq!(target.attempt, 1);
        assert_eq!(servers.connecting(), None);
    }

    #[test]
    fn keeps_a_form_that_it_gets() {
        let form = Form {
            address: "chat.example.org".to_owned(),
            name: "ana".to_owned(),
            problem: Some(refused(Disconnect::Closed)),
        };

        let servers = Servers::with(form);

        assert_eq!(servers.form().address, "chat.example.org");
        assert_eq!(servers.form().problem, Some(refused(Disconnect::Closed)));
    }

    #[test]
    fn each_problem_has_a_text() {
        let problems = [
            (
                Problem::Address(DestinationError::Empty),
                Text::ProblemAddressEmpty,
            ),
            (
                Problem::Address(DestinationError::Invalid),
                Text::ProblemAddressInvalid,
            ),
            (
                Problem::Address(DestinationError::InvalidPort),
                Text::ProblemAddressPort,
            ),
            (
                Problem::Address(DestinationError::InvalidLink),
                Text::ProblemAddressLink,
            ),
            (
                Problem::Address(DestinationError::NotLocal),
                Text::ProblemAddressNotLocal,
            ),
            (
                Problem::Name(NameError::InvalidCharacter),
                Text::ProblemNameCharacters,
            ),
            (Problem::Name(NameError::TooShort), Text::ProblemNameShort),
            (Problem::Name(NameError::TooLong), Text::ProblemNameLong),
            (
                Problem::Disconnected(Disconnect::Unreachable),
                Text::ProblemUnreachable,
            ),
            (
                Problem::Disconnected(Disconnect::Closed),
                Text::ProblemClosed,
            ),
            (
                Problem::Disconnected(Disconnect::Protocol),
                Text::ProblemProtocol,
            ),
            (
                Problem::Disconnected(Disconnect::FalseServer),
                Text::ProblemFalseServer,
            ),
            (
                Problem::Disconnected(Disconnect::DifferentServer),
                Text::ProblemDifferentServer,
            ),
            (
                Problem::Disconnected(Disconnect::MissingParts),
                Text::ProblemMissingParts,
            ),
            (
                Problem::Disconnected(Disconnect::NameRequired),
                Text::ProblemNameRequired,
            ),
            (
                Problem::Disconnected(Disconnect::InvitationRequired),
                Text::ProblemInvitationRequired,
            ),
            (
                Problem::Disconnected(Disconnect::UnknownCertificate(fingerprint(1))),
                Text::CertificateUnknown,
            ),
            (
                Problem::Disconnected(Disconnect::ChangedCertificate(fingerprint(1))),
                Text::CertificateChanged,
            ),
            (
                Problem::Disconnected(Disconnect::Refused(ErrorCode::NameTaken)),
                Text::ErrorNameTaken,
            ),
            (Problem::NotSaved, Text::ServersNotSaved),
        ];
        for (problem, text) in problems {
            assert_eq!(problem.text(), text, "{problem:?}");
        }
    }
}

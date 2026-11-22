use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

macro_rules! texts {
    ($($variant:ident => $key:literal,)+) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Text {
            $($variant,)+
        }

        impl Text {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            #[must_use]
            pub fn key(self) -> &'static str {
                match self {
                    $(Self::$variant => $key,)+
                }
            }
        }
    };
}

texts! {
    StartNoHome => "start-no-home",
    StartProfileInUse => "start-profile-in-use",
    StartProfileDamaged => "start-profile-damaged",
    StartProfileFiles => "start-profile-files",
    ProfileFirstName => "profile-first-name",
    ServersTitle => "servers-title",
    ServersAddTitle => "servers-add-title",
    ServersOpenButton => "servers-open-button",
    ServersRemoveButton => "servers-remove-button",
    ServersNotSaved => "servers-not-saved",
    ConnectServerLabel => "connect-server-label",
    ConnectNameLabel => "connect-name-label",
    ConnectButton => "connect-button",
    ConnectCancelButton => "connect-cancel-button",
    ConnectProgress => "connect-progress",

    ProblemAddressEmpty => "problem-address-empty",
    ProblemAddressInvalid => "problem-address-invalid",
    ProblemAddressPort => "problem-address-port",
    ProblemAddressLink => "problem-address-link",
    ProblemAddressNotLocal => "problem-address-not-local",
    ProblemNameCharacters => "problem-name-characters",
    ProblemNameShort => "problem-name-short",
    ProblemNameLong => "problem-name-long",
    ProblemUnreachable => "problem-unreachable",
    ProblemClosed => "problem-closed",
    ProblemProtocol => "problem-protocol",
    ProblemFalseServer => "problem-false-server",
    ProblemDifferentServer => "problem-different-server",
    ProblemMissingParts => "problem-missing-parts",
    ProblemNameRequired => "problem-name-required",
    ProblemInvitationRequired => "problem-invitation-required",
    CertificateUnknown => "certificate-unknown",
    CertificateUnknownText => "certificate-unknown-text",
    CertificateChanged => "certificate-changed",
    CertificateChangedText => "certificate-changed-text",
    CertificateFingerprintLabel => "certificate-fingerprint-label",
    CertificateAcceptButton => "certificate-accept-button",
    ErrorUnsupportedProtocol => "error-unsupported-protocol",
    ErrorWrongAddress => "error-wrong-address",
    ErrorAccountExists => "error-account-exists",
    ErrorInvalidInvitation => "error-invalid-invitation",
    ErrorServer => "error-server",
    ErrorNotPermitted => "error-not-permitted",
    ErrorTooSoon => "error-too-soon",
    ErrorInvalidName => "error-invalid-name",
    ErrorNameTaken => "error-name-taken",
    ErrorNoSuchChannel => "error-no-such-channel",
    ErrorEmptyText => "error-empty-text",
    ErrorTextTooLong => "error-text-too-long",
    ErrorOther => "error-other",
    ChatStatus => "chat-status",
    ChatLeaveButton => "chat-leave-button",
    ChatInputPlaceholder => "chat-input-placeholder",
    ChatJoined => "chat-joined",
    ChatLeft => "chat-left",
    ChatRenamed => "chat-renamed",
    ChatNotSent => "chat-not-sent",
    ChatServerNotSaved => "chat-server-not-saved",
    ChatRequestNotSent => "chat-request-not-sent",
    ChatInvitationsButton => "chat-invitations-button",
    ChatNameButton => "chat-name-button",
    ChatBackButton => "chat-back-button",
    InvitationsTitle => "invitations-title",
    InvitationsEndsLabel => "invitations-ends-label",
    InvitationsLifetimeDay => "invitations-lifetime-day",
    InvitationsLifetimeWeek => "invitations-lifetime-week",
    InvitationsLifetimeMonth => "invitations-lifetime-month",
    InvitationsLifetimeForever => "invitations-lifetime-forever",
    InvitationsUsesLabel => "invitations-uses-label",
    InvitationsUsesNoLimit => "invitations-uses-no-limit",
    InvitationsNewButton => "invitations-new-button",
    InvitationsLoading => "invitations-loading",
    InvitationsEmpty => "invitations-empty",
    InvitationsEnd => "invitations-end",
    InvitationsUsed => "invitations-used",
    InvitationsUsedOf => "invitations-used-of",
    InvitationsCopyButton => "invitations-copy-button",
    InvitationsCopied => "invitations-copied",
    InvitationsCancelButton => "invitations-cancel-button",
    AccountTitle => "account-title",
    AccountNameLabel => "account-name-label",
    AccountNote => "account-note",
    AccountRenameButton => "account-rename-button",
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Spanish,
}

impl Language {
    #[must_use]
    pub fn from_locale(locale: Option<&str>) -> Self {
        let language = locale
            .unwrap_or_default()
            .split(['-', '_', '.'])
            .next()
            .unwrap_or_default();
        if language.eq_ignore_ascii_case("es") {
            Self::Spanish
        } else {
            Self::English
        }
    }

    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        [Self::English, Self::Spanish]
            .into_iter()
            .find(|language| language.tag() == tag)
    }

    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Spanish => "es-419",
        }
    }

    fn catalog(self) -> &'static str {
        match self {
            Self::English => include_str!("../i18n/en/post.ftl"),
            Self::Spanish => include_str!("../i18n/es-419/post.ftl"),
        }
    }
}

pub struct Strings {
    language: Language,
    bundle: FluentBundle<FluentResource>,
}

impl Strings {
    #[must_use]
    pub fn new(language: Language) -> Self {
        let identifier: LanguageIdentifier = language
            .tag()
            .parse()
            .expect("each language has a valid tag");
        let resource = FluentResource::try_new(language.catalog().to_owned())
            .unwrap_or_else(|_| panic!("the {} catalog has a syntax error", language.tag()));
        let mut bundle = FluentBundle::new(vec![identifier]);
        bundle.set_use_isolating(false);
        bundle
            .add_resource(resource)
            .expect("a catalog has each text only one time");
        Self { language, bundle }
    }

    #[must_use]
    pub fn language(&self) -> Language {
        self.language
    }

    #[must_use]
    pub fn text(&self, text: Text) -> String {
        self.format(text, None)
    }

    #[must_use]
    pub fn with(&self, text: Text, variable: &str, value: &str) -> String {
        self.with_all(text, &[(variable, value)])
    }

    #[must_use]
    pub fn with_all(&self, text: Text, values: &[(&str, &str)]) -> String {
        let mut arguments = FluentArgs::new();
        for (variable, value) in values {
            arguments.set(*variable, *value);
        }
        self.format(text, Some(&arguments))
    }

    fn format(&self, text: Text, arguments: Option<&FluentArgs>) -> String {
        let pattern = self
            .bundle
            .get_message(text.key())
            .and_then(|message| message.value());
        let Some(pattern) = pattern else {
            return text.key().to_owned();
        };
        let mut errors = Vec::new();
        self.bundle
            .format_pattern(pattern, arguments, &mut errors)
            .into_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{Language, Strings, Text};

    const LANGUAGES: [Language; 2] = [Language::English, Language::Spanish];

    fn catalog(language: Language) -> BTreeMap<String, String> {
        language
            .catalog()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let (key, value) = line
                    .split_once(" = ")
                    .unwrap_or_else(|| panic!("not a simple message line: {line}"));
                (key.to_owned(), value.to_owned())
            })
            .collect()
    }

    fn variables(value: &str) -> BTreeSet<String> {
        value
            .split("{ $")
            .skip(1)
            .map(|rest| {
                rest.split_once(" }")
                    .unwrap_or_else(|| panic!("a variable does not close: {value}"))
                    .0
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn selects_spanish_for_each_spanish_locale() {
        for locale in ["es", "es-MX", "es-419", "es_AR.UTF-8", "ES-co", "es-ES"] {
            assert_eq!(
                Language::from_locale(Some(locale)),
                Language::Spanish,
                "{locale}"
            );
        }
    }

    #[test]
    fn selects_english_for_each_other_locale() {
        for locale in [
            "en", "en-US", "pt-BR", "est", "eo", "", "e", "C", "-es", "esp-MX",
        ] {
            assert_eq!(
                Language::from_locale(Some(locale)),
                Language::English,
                "{locale}"
            );
        }
        assert_eq!(Language::from_locale(None), Language::English);
    }

    #[test]
    fn each_language_has_a_tag() {
        assert_eq!(Language::English.tag(), "en");
        assert_eq!(Language::Spanish.tag(), "es-419");
    }

    #[test]
    fn reads_a_language_from_its_tag() {
        assert_eq!(Language::from_tag("en"), Some(Language::English));
        assert_eq!(Language::from_tag("es-419"), Some(Language::Spanish));
        assert_eq!(Language::from_tag("es"), None);
        assert_eq!(Language::from_tag(""), None);
    }

    #[test]
    fn each_catalog_has_each_text_and_no_other_text() {
        let expected: BTreeSet<String> =
            Text::ALL.iter().map(|text| text.key().to_owned()).collect();
        assert_eq!(expected.len(), Text::ALL.len(), "two texts have one key");

        for language in LANGUAGES {
            let keys: BTreeSet<String> = catalog(language).into_keys().collect();
            assert_eq!(keys, expected, "{language:?}");
        }
    }

    #[test]
    fn the_catalogs_use_the_same_variables() {
        let english = catalog(Language::English);
        let spanish = catalog(Language::Spanish);

        for (key, value) in &english {
            assert_eq!(variables(value), variables(&spanish[key]), "{key}");
        }
    }

    #[test]
    fn each_text_has_a_value_in_each_language() {
        for language in LANGUAGES {
            let strings = Strings::new(language);
            for text in Text::ALL {
                let value = strings.with(*text, "name", "ben");
                assert!(!value.is_empty(), "{language:?} {text:?}");
                assert_ne!(value, text.key(), "{language:?} {text:?}");
            }
        }
    }

    #[test]
    fn gives_a_text_in_english_and_in_spanish() {
        let english = Strings::new(Language::English);
        let spanish = Strings::new(Language::Spanish);

        assert_eq!(english.text(Text::ConnectButton), "Connect");
        assert_eq!(spanish.text(Text::ConnectButton), "Conectar");
        assert_eq!(english.language(), Language::English);
        assert_eq!(spanish.language(), Language::Spanish);
    }

    #[test]
    fn puts_a_value_into_a_text_without_marks_around_it() {
        let english = Strings::new(Language::English);
        let spanish = Strings::new(Language::Spanish);

        assert_eq!(english.with(Text::ChatJoined, "name", "ben"), "ben joined");
        assert_eq!(spanish.with(Text::ChatJoined, "name", "ben"), "ben entró");
        assert_eq!(
            english.with(Text::ChatInputPlaceholder, "channel", "lobby"),
            "Message #lobby"
        );
    }

    #[test]
    fn puts_two_values_into_a_text() {
        let english = Strings::new(Language::English);
        let spanish = Strings::new(Language::Spanish);
        let values = [("old", "ana"), ("new", "anna")];

        assert_eq!(
            english.with_all(Text::ChatRenamed, &values),
            "ana is now anna"
        );
        assert_eq!(
            spanish.with_all(Text::ChatRenamed, &values),
            "ana ahora es anna"
        );
    }

    #[test]
    fn shows_the_name_of_a_variable_that_has_no_value() {
        let english = Strings::new(Language::English);

        assert_eq!(english.text(Text::ChatJoined), "{$name} joined");
    }
}

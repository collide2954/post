use std::hash::{Hash, Hasher};

use iced::keyboard::{self, Key, key::Named};
use iced::widget::operation;
use iced::{
    Application, Element, Pixels, Program, Settings, Size, Subscription, Task, Theme, theme, window,
};
use jiff::tz::TimeZone;
use post_client::{Home, Identity, Profile, Profiles, Seed};

use crate::app::{App, Effect, Message, StartProblem, Target};
use crate::i18n::{Language, Strings, Text};
use crate::palette::Mode;
use crate::view::{self, Window};

pub struct Shell {
    window: Window,
    profile: Option<Profile>,
}

struct SignIn {
    target: Target,
    seed: Seed,
}

impl Hash for SignIn {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.target.attempt.hash(state);
    }
}

pub fn application(
    language: Language,
    home: Option<Home>,
) -> Application<impl Program<State = Shell, Message = Message, Theme = Theme>> {
    let mut application = iced::application(move || boot(language, home.clone()), update, view)
        .title("Post")
        .theme(window_theme)
        .subscription(subscription)
        .settings(Settings {
            default_text_size: Pixels(14.0),
            ..Settings::default()
        })
        .default_font(view::TEXT_FONT)
        .window(window::Settings {
            size: Size::new(900.0, 600.0),
            min_size: Some(Size::new(480.0, 320.0)),
            ..window::Settings::default()
        });
    for font in view::FONTS {
        application = application.font(font);
    }
    application
}

fn boot(language: Language, home: Option<Home>) -> (Shell, Task<Message>) {
    let strings = Strings::new(language);
    let profile = home.ok_or(StartProblem::NoHome).and_then(|home| {
        Profiles::new(home)
            .start(&strings.text(Text::ProfileFirstName))
            .map_err(StartProblem::from)
    });
    let (app, profile) = match profile {
        Ok(profile) => (
            App::new(Mode::Light, profile.servers().to_vec()),
            Some(profile),
        ),
        Err(problem) => (App::stopped(Mode::Light, problem), None),
    };
    let shell = Shell {
        window: Window {
            app,
            strings,
            zone: TimeZone::system(),
        },
        profile,
    };
    let tasks = Task::batch([
        iced::system::theme().map(mode_changed),
        operation::focus(view::NAME_FIELD),
    ]);
    (shell, tasks)
}

fn update(shell: &mut Shell, message: Message) -> Task<Message> {
    match shell.window.app.update(message) {
        Effect::None => Task::none(),
        Effect::FocusName => operation::focus(view::NAME_FIELD),

        Effect::FocusNext => operation::focus_next(),
        Effect::FocusPrevious => operation::focus_previous(),
        Effect::Connected(server) => {
            let saved = shell
                .profile
                .as_mut()
                .is_some_and(|profile| profile.remember(server).is_ok());
            if !saved {
                shell.window.app.update(Message::ServerNotSaved);
            }
            operation::focus(view::DRAFT_FIELD)
        }
    }
}

fn view(shell: &Shell) -> Element<'_, Message> {
    view::view(&shell.window)
}

fn window_theme(shell: &Shell) -> Theme {
    view::theme(shell.window.app.mode())
}

fn subscription(shell: &Shell) -> Subscription<Message> {
    let connection = match (shell.window.app.target(), &shell.profile) {
        (Some(target), Some(profile)) => {
            let sign_in = SignIn {
                target: target.clone(),
                seed: profile.seed().clone(),
            };
            Subscription::run_with(sign_in, |sign_in| {
                let identity = Identity {
                    seed: sign_in.seed.clone(),
                    name: sign_in.target.name.clone(),
                };
                post_client::sign_in(
                    sign_in.target.destination.clone(),
                    identity,
                    sign_in.target.known,
                )
            })
            .with(target.attempt)
            .map(|(attempt, event)| Message::Connection(attempt, event))
        }
        _ => Subscription::none(),
    };
    Subscription::batch([
        connection,
        iced::system::theme_changes().map(mode_changed),
        keyboard::listen().filter_map(|event| focus_key(&event)),
    ])
}

fn mode_changed(mode: theme::Mode) -> Message {
    Message::ModeChanged(match mode {
        theme::Mode::Dark => Mode::Dark,
        theme::Mode::Light | theme::Mode::None => Mode::Light,
    })
}

fn focus_key(event: &keyboard::Event) -> Option<Message> {
    match event {
        keyboard::Event::KeyPressed {
            key: Key::Named(Named::Tab),
            modifiers,
            ..
        } => Some(if modifiers.shift() {
            Message::FocusPrevious
        } else {
            Message::FocusNext
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use iced::keyboard::key::{self, Named};
    use iced::keyboard::{self, Key, Location, Modifiers};
    use iced::theme;

    use super::{focus_key, mode_changed};
    use crate::app::Message;
    use crate::palette::Mode;

    fn pressed(key: Key, modifiers: Modifiers) -> keyboard::Event {
        keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: key::Physical::Unidentified(key::NativeCode::Unidentified),
            location: Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        }
    }

    #[test]
    fn the_tab_key_moves_the_focus_to_the_next_field() {
        let event = pressed(Key::Named(Named::Tab), Modifiers::empty());

        assert!(matches!(focus_key(&event), Some(Message::FocusNext)));
    }

    #[test]
    fn the_shift_and_tab_keys_move_the_focus_to_the_field_before() {
        let event = pressed(Key::Named(Named::Tab), Modifiers::SHIFT);

        assert!(matches!(focus_key(&event), Some(Message::FocusPrevious)));
    }

    #[test]
    fn a_different_key_does_not_move_the_focus() {
        let enter = pressed(Key::Named(Named::Enter), Modifiers::empty());
        let released = keyboard::Event::KeyReleased {
            key: Key::Named(Named::Tab),
            modified_key: Key::Named(Named::Tab),
            physical_key: key::Physical::Unidentified(key::NativeCode::Unidentified),
            location: Location::Standard,
            modifiers: Modifiers::empty(),
        };

        assert!(focus_key(&enter).is_none());
        assert!(focus_key(&released).is_none());
    }

    #[test]
    fn the_application_obeys_the_mode_of_the_system() {
        let modes = [
            (theme::Mode::Dark, Mode::Dark),
            (theme::Mode::Light, Mode::Light),
            (theme::Mode::None, Mode::Light),
        ];
        for (system, expected) in modes {
            assert!(
                matches!(mode_changed(system), Message::ModeChanged(mode) if mode == expected),
                "{system:?}"
            );
        }
    }
}

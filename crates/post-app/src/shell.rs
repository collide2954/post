use iced::keyboard::{self, Key, key::Named};
use iced::widget::operation;
use iced::{
    Application, Pixels, Program, Settings, Size, Subscription, Task, Theme, theme, window,
};
use jiff::tz::TimeZone;

use crate::app::{App, Effect, Message};
use crate::i18n::{Language, Strings};
use crate::palette::Mode;
use crate::view::{self, Window};

pub fn application(
    language: Language,
) -> Application<impl Program<State = Window, Message = Message, Theme = Theme>> {
    let mut application = iced::application(move || boot(language), update, view::view)
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

fn boot(language: Language) -> (Window, Task<Message>) {
    let window = Window {
        app: App::new(Mode::Light),
        strings: Strings::new(language),
        zone: TimeZone::system(),
    };
    let tasks = Task::batch([
        iced::system::theme().map(mode_changed),
        operation::focus(view::NAME_FIELD),
    ]);
    (window, tasks)
}

fn update(window: &mut Window, message: Message) -> Task<Message> {
    match window.app.update(message) {
        Effect::None => Task::none(),
        Effect::FocusName => operation::focus(view::NAME_FIELD),
        Effect::FocusDraft => operation::focus(view::DRAFT_FIELD),
        Effect::FocusNext => operation::focus_next(),
        Effect::FocusPrevious => operation::focus_previous(),
    }
}

fn window_theme(window: &Window) -> Theme {
    view::theme(window.app.mode())
}

fn subscription(window: &Window) -> Subscription<Message> {
    let connection = window
        .app
        .target()
        .map_or_else(Subscription::none, |target| {
            Subscription::run_with(target.clone(), |target| {
                post_client::connect(target.address.clone(), target.name.clone())
            })
            .with(target.attempt)
            .map(|(attempt, event)| Message::Connection(attempt, event))
        });
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

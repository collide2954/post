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

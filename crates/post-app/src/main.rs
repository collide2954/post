#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use iced::keyboard::{self, Key, key::Named};
use iced::widget::operation;
use iced::{Pixels, Settings, Size, Subscription, Task, Theme, theme, window};
use jiff::tz::TimeZone;
use post_app::app::{App, Effect, Message};
use post_app::i18n::{Language, Strings};
use post_app::palette::Mode;
use post_app::view::{self, Window};

const LANGUAGE_VARIABLE: &str = "POST_LANGUAGE";

fn main() -> iced::Result {
    let mut application = iced::application(boot, update, view::view)
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
    application.run()
}

fn boot() -> (Window, Task<Message>) {
    let language = std::env::var(LANGUAGE_VARIABLE)
        .ok()
        .and_then(|tag| Language::from_tag(&tag))
        .unwrap_or_else(|| Language::from_locale(sys_locale::get_locale().as_deref()));
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

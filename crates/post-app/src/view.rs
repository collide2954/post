mod chat;
mod servers;
mod settings;
mod widgets;

use std::sync::LazyLock;

use iced::{Element, Font, Theme, font};
use jiff::tz::TimeZone;

use self::chat::chat_screen;
use self::servers::{certificate_screen, servers_screen, stopped_screen};
use self::settings::settings_page;
use self::widgets::color;
use crate::app::{App, Message, Page, Screen};
use crate::i18n::Strings;
use crate::palette::{Mode, Palette};
pub const ADDRESS_FIELD: &str = "address";
pub const NAME_FIELD: &str = "name";
pub const DRAFT_FIELD: &str = "draft";
pub const PANEL_FIELD: &str = "panel";

pub const TEXT_FONT: Font = Font::with_name("Inter");
const STRONG_FONT: Font = Font {
    weight: font::Weight::Semibold,
    ..Font::with_name("Inter")
};
const MONO_FONT: Font = Font::with_name("JetBrains Mono");

pub const FONTS: [&[u8]; 3] = [
    include_bytes!("../fonts/Inter-Regular.ttf"),
    include_bytes!("../fonts/Inter-SemiBold.ttf"),
    include_bytes!("../fonts/JetBrainsMono-Regular.ttf"),
];

const TEXT_SIZE: f32 = 14.0;
const SMALL_SIZE: f32 = 12.0;
const LINE_HEIGHT: f32 = 21.0;
const NAME_CHARACTER_WIDTH: f32 = 8.5;
const PAGE_WIDTH: f32 = 420.0;
const BAR_HEIGHT: f32 = 42.0;
const STATUS_WIDTH: f32 = 740.0;

pub struct Window {
    pub app: App,
    pub strings: Strings,
    pub zone: TimeZone,
    pub settings_file: Option<String>,
}

#[must_use]
pub fn theme(mode: Mode) -> Theme {
    static LIGHT: LazyLock<Theme> = LazyLock::new(|| custom_theme("Post Light", Mode::Light));
    static DARK: LazyLock<Theme> = LazyLock::new(|| custom_theme("Post Dark", Mode::Dark));
    match mode {
        Mode::Light => LIGHT.clone(),
        Mode::Dark => DARK.clone(),
    }
}

fn custom_theme(name: &'static str, mode: Mode) -> Theme {
    let palette = Palette::of(mode);
    Theme::custom(
        name,
        iced::theme::Palette {
            background: color(palette.background),
            text: color(palette.text),
            primary: color(palette.accent),
            success: color(palette.accent),
            warning: color(palette.danger),
            danger: color(palette.danger),
        },
    )
}

pub fn view(window: &Window) -> Element<'_, Message> {
    let palette = Palette::of(window.app.mode());
    let strings = &window.strings;
    if window.app.page() == Some(Page::Settings) {
        return settings_page(window, palette);
    }
    match window.app.screen() {
        Screen::Stopped(problem) => stopped_screen(strings, problem, palette),
        Screen::Servers(servers) => match servers.question() {
            Some(question) => certificate_screen(strings, question, palette),
            None => servers_screen(strings, servers, window.app.servers(), palette),
        },
        Screen::Chat(chat) => chat_screen(window, chat, palette),
    }
}

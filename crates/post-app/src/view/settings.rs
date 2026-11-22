use iced::widget::text::{LineHeight, Wrapping};
use iced::widget::{Space, column, row, text};
use iced::{Alignment, Element, Pixels};

use super::widgets::{
    Choice, bar, check, choice, color, label, link_button, panel_page, paragraph, pill,
};
use super::{LINE_HEIGHT, SMALL_SIZE, Window};
use crate::app::{self, Message};
use crate::i18n::{Language, Strings, Text};
use crate::palette::Palette;
use crate::settings::{self, Clock, Settings};

pub(super) fn settings_page(window: &Window, palette: Palette) -> Element<'_, Message> {
    let strings = &window.strings;
    let current = window.app.settings();
    let back = link_button(
        strings.text(Text::NavBackButton),
        Some(Message::Back),
        palette,
    );
    let seconds = check(
        strings.text(Text::SettingsSeconds),
        current.seconds,
        |seconds| Message::Settings(app::settings::Message::SecondsToggled(seconds)),
        palette,
    );
    let console = check(
        strings.text(Text::SettingsConsole),
        current.console,
        |console| Message::Settings(app::settings::Message::ConsoleToggled(console)),
        palette,
    );

    let mut content = column![
        label(strings.text(Text::SettingsLanguageLabel), palette),
        language_choices(strings, *current, palette),
        Space::new().height(8),
        label(strings.text(Text::SettingsClockLabel), palette),
        clock_choices(strings, *current, palette),
        seconds,
        Space::new().height(8),
        label(strings.text(Text::SettingsTextSizeLabel), palette),
        size_steps(strings, *current, palette),
        Space::new().height(8),
        label(strings.text(Text::SettingsDeveloperLabel), palette),
        console,
        Space::new().height(8),
    ];
    if let Some(problem) = window.app.settings_problem() {
        let problem = settings_problem(strings, problem);
        content = content.push(paragraph(problem, palette.danger));
    }
    if let Some(path) = &window.settings_file {
        content = content.push(
            text(strings.with(Text::SettingsFile, "path", path))
                .size(SMALL_SIZE)
                .line_height(LineHeight::Absolute(Pixels(LINE_HEIGHT)))
                .wrapping(Wrapping::WordOrGlyph)
                .color(color(palette.muted)),
        );
    }
    column![
        bar(strings.text(Text::SettingsTitle), None, row![back], palette),
        panel_page(content),
    ]
    .into()
}

pub(super) fn language_choices<'a>(
    strings: &Strings,
    current: Settings,
    palette: Palette,
) -> Element<'a, Message> {
    let languages = [
        (None, Text::SettingsLanguageSystem),
        (Some(Language::English), Text::SettingsLanguageEnglish),
        (Some(Language::Spanish), Text::SettingsLanguageSpanish),
    ]
    .into_iter()
    .map(|(language, name)| {
        let select = app::settings::Message::LanguageSelected(language);
        choice(
            strings.text(name),
            current.language == language,
            Message::Settings(select),
            palette,
        )
    });
    row(languages).spacing(6).into()
}

pub(super) fn clock_choices<'a>(
    strings: &Strings,
    current: Settings,
    palette: Palette,
) -> Element<'a, Message> {
    let clocks = [
        (Clock::Hours24, Text::SettingsClock24),
        (Clock::Hours12, Text::SettingsClock12),
    ]
    .into_iter()
    .map(|(clock, name)| {
        let select = app::settings::Message::ClockSelected(clock);
        choice(
            strings.text(name),
            current.clock == clock,
            Message::Settings(select),
            palette,
        )
    });
    row(clocks).spacing(6).into()
}

pub(super) fn size_steps<'a>(
    strings: &Strings,
    current: Settings,
    palette: Palette,
) -> Element<'a, Message> {
    let step = |name: Text, possible: bool, message: app::settings::Message| {
        let state = if possible {
            Choice::Available(Message::Settings(message))
        } else {
            Choice::Unavailable
        };
        pill(strings.text(name), state, palette)
    };
    row![
        step(
            Text::SettingsSmallerButton,
            current.text_size > Settings::MIN_TEXT_SIZE,
            app::settings::Message::Smaller
        ),
        text(format!("{}%", current.text_size)).width(48).center(),
        step(
            Text::SettingsLargerButton,
            current.text_size < Settings::MAX_TEXT_SIZE,
            app::settings::Message::Larger
        ),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

pub(super) fn settings_problem(strings: &Strings, problem: app::settings::Problem) -> String {
    match problem {
        app::settings::Problem::File(settings::Problem::Syntax) => {
            strings.text(Text::SettingsProblemSyntax)
        }
        app::settings::Problem::File(settings::Problem::Value(key)) => {
            strings.with(Text::SettingsProblemValue, "key", key)
        }
        app::settings::Problem::NotSaved => strings.text(Text::SettingsNotSaved),
    }
}

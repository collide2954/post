use iced::widget::text::LineHeight;
use iced::widget::{
    Column, Row, Space, button, checkbox, column, container, row, rule, scrollable, space, text,
    text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Fill, Pixels, border};

use super::{LINE_HEIGHT, PAGE_WIDTH, SMALL_SIZE, STRONG_FONT, TEXT_FONT, TEXT_SIZE};
use crate::app::{Message, Page};
use crate::i18n::{Strings, Text};
use crate::palette::{Palette, Rgb};

pub(super) fn page(content: Column<'_, Message>) -> Element<'_, Message> {
    let content = content.spacing(8).max_width(PAGE_WIDTH);
    container(scrollable(container(content).center_x(Fill).padding(24)))
        .center_y(Fill)
        .into()
}

pub(super) fn panel_page(content: Column<'_, Message>) -> Element<'_, Message> {
    let content = content.spacing(8).width(Fill).max_width(PAGE_WIDTH);
    scrollable(container(content).center_x(Fill).padding(24))
        .height(Fill)
        .into()
}

pub(super) fn title<'a>() -> Element<'a, Message> {
    column![
        text("Post").font(STRONG_FONT).size(28),
        Space::new().height(12)
    ]
    .into()
}

pub(super) fn heading<'a>(content: String) -> Element<'a, Message> {
    column![
        text(content).font(STRONG_FONT).size(18),
        Space::new().height(8)
    ]
    .into()
}

pub(super) fn paragraph<'a>(content: String, tone: Rgb) -> Element<'a, Message> {
    text(content)
        .line_height(LineHeight::Absolute(Pixels(LINE_HEIGHT)))
        .color(color(tone))
        .into()
}

pub(super) fn label<'a>(content: String, palette: Palette) -> Element<'a, Message> {
    text(content)
        .size(SMALL_SIZE)
        .font(STRONG_FONT)
        .color(color(palette.muted))
        .into()
}

pub(super) fn divider<'a>(palette: Palette) -> Element<'a, Message> {
    rule::horizontal(1)
        .style(move |theme| rule::Style {
            color: color(palette.rule),
            ..rule::default(theme)
        })
        .into()
}

pub(super) fn bar(
    heading: String,
    status: Option<String>,
    links: Row<'_, Message>,
    palette: Palette,
) -> Element<'_, Message> {
    let mut bar = row![
        text(heading).font(STRONG_FONT).size(15),
        space::horizontal()
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .padding([8, 16]);
    if let Some(status) = status {
        bar = bar.push(text(status).size(13).color(color(palette.muted)));
    }
    column![bar.push(links), divider(palette)].into()
}

pub(super) fn page_link<'a>(
    strings: &Strings,
    page: Page,
    padding: [u16; 2],
    palette: Palette,
) -> Element<'a, Message> {
    let name = match page {
        Page::Settings => Text::NavSettingsButton,
        Page::Profiles => Text::NavProfilesButton,
    };
    link(
        strings.text(name),
        Some(Message::Open(page)),
        padding,
        palette,
    )
}

pub(super) fn danger_button<'a>(
    content: String,
    message: Option<Message>,
    palette: Palette,
) -> Element<'a, Message> {
    button(text(content).font(STRONG_FONT))
        .padding([8, 18])
        .on_press_maybe(message)
        .style(move |_, status| {
            let (background, text_color) = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    (mix(palette.danger, palette.text, 0.18), palette.background)
                }
                button::Status::Active => (color(palette.danger), palette.background),
                button::Status::Disabled => (color(palette.rule), palette.muted),
            };
            button::Style {
                background: Some(Background::Color(background)),
                text_color: color(text_color),
                border: border::rounded(4),
                ..button::Style::default()
            }
        })
        .into()
}

pub(super) fn primary_button<'a>(
    content: String,
    message: Message,
    palette: Palette,
) -> Element<'a, Message> {
    button(text(content).font(STRONG_FONT))
        .padding([8, 18])
        .on_press(message)
        .style(move |_, status| {
            let background = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    mix(palette.accent, palette.text, 0.18)
                }
                button::Status::Active | button::Status::Disabled => color(palette.accent),
            };
            button::Style {
                background: Some(Background::Color(background)),
                text_color: color(palette.on_accent),
                border: border::rounded(4),
                ..button::Style::default()
            }
        })
        .into()
}

pub(super) fn link_button<'a>(
    content: String,
    message: Option<Message>,
    palette: Palette,
) -> Element<'a, Message> {
    link(content, message, [4, 8], palette)
}

pub(super) fn link<'a>(
    content: String,
    message: Option<Message>,
    padding: [u16; 2],
    palette: Palette,
) -> Element<'a, Message> {
    button(text(content).size(13))
        .padding(padding)
        .on_press_maybe(message)
        .style(move |_, status| {
            let text_color = match status {
                button::Status::Hovered | button::Status::Pressed => color(palette.text),
                button::Status::Active => color(palette.accent),
                button::Status::Disabled => color(palette.muted),
            };
            button::Style {
                background: None,
                text_color,
                ..button::Style::default()
            }
        })
        .into()
}

pub(super) enum Choice {
    Selected,
    Available(Message),
    Unavailable,
}

pub(super) fn choice<'a>(
    content: String,
    selected: bool,
    message: Message,
    palette: Palette,
) -> Element<'a, Message> {
    let state = if selected {
        Choice::Selected
    } else {
        Choice::Available(message)
    };
    pill(content, state, palette)
}

pub(super) fn pill<'a>(content: String, state: Choice, palette: Palette) -> Element<'a, Message> {
    let selected = matches!(state, Choice::Selected);
    let font = if selected { STRONG_FONT } else { TEXT_FONT };
    let message = match state {
        Choice::Available(message) => Some(message),
        Choice::Selected | Choice::Unavailable => None,
    };
    button(text(content).size(13).font(font))
        .padding([4, 10])
        .on_press_maybe(message)
        .style(move |_, status| {
            let (text_color, border_color) = match status {
                button::Status::Disabled if selected => (palette.text, palette.text),
                button::Status::Disabled => (palette.muted, palette.rule),
                button::Status::Hovered | button::Status::Pressed => (palette.text, palette.rule),
                button::Status::Active => (palette.accent, palette.rule),
            };
            button::Style {
                background: None,
                text_color: color(text_color),
                border: Border {
                    color: color(border_color),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..button::Style::default()
            }
        })
        .into()
}

pub(super) fn check<'a>(
    content: String,
    checked: bool,
    on_toggle: fn(bool) -> Message,
    palette: Palette,
) -> Element<'a, Message> {
    checkbox(checked)
        .label(content)
        .on_toggle(on_toggle)
        .size(16)
        .spacing(8)
        .text_size(TEXT_SIZE)
        .style(move |_, status| {
            let (checked, hovered) = match status {
                checkbox::Status::Active { is_checked }
                | checkbox::Status::Disabled { is_checked } => (is_checked, false),
                checkbox::Status::Hovered { is_checked } => (is_checked, true),
            };
            let fill = if checked {
                palette.accent
            } else {
                palette.background
            };
            let edge = if checked || hovered {
                palette.accent
            } else {
                palette.muted
            };
            checkbox::Style {
                background: Background::Color(color(fill)),
                icon_color: color(palette.on_accent),
                border: Border {
                    color: color(edge),
                    width: 1.0,
                    radius: 3.0.into(),
                },
                text_color: Some(color(palette.text)),
            }
        })
        .into()
}

pub(super) fn field_style(
    palette: Palette,
    status: text_input::Status,
    border_width: f32,
) -> text_input::Style {
    let (border_color, value) = match status {
        text_input::Status::Focused { .. } => (palette.accent, palette.text),
        text_input::Status::Active | text_input::Status::Hovered => (palette.rule, palette.text),
        text_input::Status::Disabled => (palette.rule, palette.muted),
    };
    text_input::Style {
        background: Background::Color(color(palette.background)),
        border: Border {
            color: color(border_color),
            width: border_width,
            radius: 4.0.into(),
        },
        icon: color(palette.muted),
        placeholder: color(palette.muted),
        value: color(value),
        selection: color(palette.selection),
    }
}

pub(super) fn color(rgb: Rgb) -> Color {
    Color::from_rgb8(rgb.0, rgb.1, rgb.2)
}

pub(super) fn mix(first: Rgb, second: Rgb, amount: f32) -> Color {
    let first = color(first);
    let second = color(second);
    Color {
        r: first.r + (second.r - first.r) * amount,
        g: first.g + (second.g - first.g) * amount,
        b: first.b + (second.b - first.b) * amount,
        a: 1.0,
    }
}

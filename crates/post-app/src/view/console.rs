use iced::widget::text::{LineHeight, Wrapping};
use iced::widget::{column, container, row, scrollable, space, text, text_input};
use iced::{Alignment, Element, Fill, Padding, Pixels};
use post_client::{Direction, Line};

use super::widgets::{color, divider, field_style, label, link_button};
use super::{CONSOLE_FIELD, LINE_HEIGHT, MONO_FONT, SMALL_SIZE, Window};
use crate::app::console::{self, Console};
use crate::app::{Message, settings};
use crate::i18n::Text;
use crate::palette::Palette;
use crate::timeline::precise_clock;

const HEIGHT: f32 = 220.0;
const SENT: &str = "→";
const RECEIVED: &str = "←";

pub(super) fn console_panel<'a>(
    window: &'a Window,
    console: &'a Console,
    palette: Palette,
) -> Element<'a, Message> {
    let strings = &window.strings;
    let send = |message: console::Message| Some(Message::Console(message));
    let close = Message::Settings(settings::Message::ConsoleToggled(false));
    let header = row![
        label(strings.text(Text::ConsoleTitle), palette),
        space::horizontal(),
        link_button(
            strings.text(Text::ConsoleClearButton),
            send(console::Message::Clear),
            palette
        ),
        link_button(strings.text(Text::ConsoleCloseButton), Some(close), palette),
    ]
    .align_y(Alignment::Center)
    .padding(Padding {
        top: 2.0,
        right: 8.0,
        bottom: 2.0,
        left: 16.0,
    });

    let lines: Element<'a, Message> = if console.lines().is_empty() {
        container(
            text(strings.text(Text::ConsoleEmpty))
                .size(SMALL_SIZE)
                .color(color(palette.muted)),
        )
        .padding([4, 16])
        .height(Fill)
        .into()
    } else {
        let lines = console
            .lines()
            .iter()
            .map(|line| console_line(window, line, palette));
        scrollable(column(lines).padding([4, 16]))
            .anchor_bottom()
            .width(Fill)
            .height(Fill)
            .into()
    };

    let field = text_input(
        &strings.text(Text::ConsoleInputPlaceholder),
        console.draft(),
    )
    .id(CONSOLE_FIELD)
    .font(MONO_FONT)
    .size(SMALL_SIZE)
    .on_input(|draft| Message::Console(console::Message::DraftChanged(draft)))
    .on_submit(Message::Console(console::Message::Send))
    .padding([6, 16])
    .style(move |_, status| field_style(palette, status, 0.0));
    let mut input = row![field].align_y(Alignment::Center);
    if console.not_sent() {
        input = input.push(
            text(strings.text(Text::ConsoleNotSent))
                .size(SMALL_SIZE)
                .color(color(palette.danger)),
        );
    }
    let input = input.push(link_button(
        strings.text(Text::ConsoleSendButton),
        send(console::Message::Send),
        palette,
    ));

    container(column![
        divider(palette),
        header,
        lines,
        divider(palette),
        input,
    ])
    .height(HEIGHT)
    .into()
}

fn console_line<'a>(window: &Window, line: &'a Line, palette: Palette) -> Element<'a, Message> {
    let line_height = LineHeight::Absolute(Pixels(LINE_HEIGHT - 3.0));
    let direction = match line.direction {
        Direction::Sent => SENT,
        Direction::Received => RECEIVED,
    };
    let small = |content: String, tone| {
        text(content)
            .font(MONO_FONT)
            .size(SMALL_SIZE)
            .line_height(line_height)
            .color(color(tone))
    };
    row![
        small(precise_clock(line.time, &window.zone), palette.muted),
        small(direction.to_owned(), palette.accent),
        text(line.text.as_str())
            .font(MONO_FONT)
            .size(SMALL_SIZE)
            .line_height(line_height)
            .wrapping(Wrapping::WordOrGlyph)
            .width(Fill),
    ]
    .spacing(8)
    .into()
}

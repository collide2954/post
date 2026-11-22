use std::sync::LazyLock;

use iced::widget::text::LineHeight;
use iced::widget::{
    Column, Space, button, column, container, row, rule, scrollable, space, text, text_input,
};
use iced::{
    Alignment, Background, Border, Color, Element, Fill, Font, Pixels, Theme, border, font,
};
use jiff::tz::TimeZone;
use post_client::{Entry, KnownServer};
use post_proto::{Invitation, LOBBY};

use crate::app::chat::{self, Account, Chat, Invitations, Lifetime, Open, Panel, Uses};
use crate::app::servers::{self, Question, Servers};
use crate::app::start::StartProblem;
use crate::app::{App, Message, Screen};
use crate::i18n::{Strings, Text};
use crate::palette::{Mode, Palette, Rgb};
use crate::timeline::{clock, date_and_clock, name_column, shows_name};

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

pub struct Window {
    pub app: App,
    pub strings: Strings,
    pub zone: TimeZone,
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
    match window.app.screen() {
        Screen::Stopped(problem) => stopped_screen(strings, problem, palette),
        Screen::Servers(servers) => match servers.question() {
            Some(question) => certificate_screen(strings, question, palette),
            None => servers_screen(strings, servers, window.app.servers(), palette),
        },
        Screen::Chat(chat) => chat_screen(window, chat, palette),
    }
}

const CONNECT: Message = Message::Servers(servers::Message::Connect);
const CANCEL: Message = Message::Servers(servers::Message::Cancel);

fn address_changed(address: String) -> Message {
    Message::Servers(servers::Message::AddressChanged(address))
}

fn name_changed(name: String) -> Message {
    Message::Servers(servers::Message::NameChanged(name))
}

fn page(content: Column<'_, Message>) -> Element<'_, Message> {
    let content = content.spacing(8).max_width(PAGE_WIDTH);
    container(scrollable(container(content).center_x(Fill).padding(24)))
        .center_y(Fill)
        .into()
}

fn title<'a>() -> Element<'a, Message> {
    column![
        text("Post").font(STRONG_FONT).size(28),
        Space::new().height(12)
    ]
    .into()
}

fn paragraph<'a>(content: String, tone: Rgb) -> Element<'a, Message> {
    text(content)
        .line_height(LineHeight::Absolute(Pixels(LINE_HEIGHT)))
        .color(color(tone))
        .into()
}

fn stopped_screen<'a>(
    strings: &Strings,
    problem: &StartProblem,
    palette: Palette,
) -> Element<'a, Message> {
    page(column![
        title(),
        paragraph(problem.describe(strings), palette.danger),
    ])
}

fn certificate_screen<'a>(
    strings: &Strings,
    question: &Question,
    palette: Palette,
) -> Element<'a, Message> {
    let (headline, explanation, tone) = if question.changed {
        (
            Text::CertificateChanged,
            Text::CertificateChangedText,
            palette.danger,
        )
    } else {
        (
            Text::CertificateUnknown,
            Text::CertificateUnknownText,
            palette.text,
        )
    };
    let fingerprint = question.fingerprint.grouped();
    let (first, second) = fingerprint.split_at(fingerprint.len() / 2);
    let line = |groups: &str| {
        text(groups.trim().to_owned())
            .font(MONO_FONT)
            .size(SMALL_SIZE)
            .line_height(LineHeight::Absolute(Pixels(LINE_HEIGHT)))
    };
    let accept = Message::Servers(servers::Message::AcceptCertificate);

    page(column![
        title(),
        text(question.target.destination.to_string()).font(STRONG_FONT),
        paragraph(strings.text(headline), tone),
        paragraph(strings.text(explanation), palette.muted),
        Space::new().height(4),
        label(strings.text(Text::CertificateFingerprintLabel), palette),
        line(first),
        line(second),
        Space::new().height(8),
        row![
            space::horizontal(),
            link_button(
                strings.text(Text::ConnectCancelButton),
                Some(CANCEL),
                palette
            ),
            primary_button(strings.text(Text::CertificateAcceptButton), accept, palette),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    ])
}

fn servers_screen<'a>(
    strings: &Strings,
    servers: &'a Servers,
    known: &'a [KnownServer],
    palette: Palette,
) -> Element<'a, Message> {
    let form = servers.form();
    let connecting = servers.connecting();
    let idle = connecting.is_none();

    let mut content = column![title()];
    if !known.is_empty() {
        content = content.push(label(strings.text(Text::ServersTitle), palette));
        for (index, server) in known.iter().enumerate() {
            content = content.push(server_row(strings, index, server, idle, palette));
        }
        content = content
            .push(Space::new().height(12))
            .push(label(strings.text(Text::ServersAddTitle), palette))
            .push(Space::new().height(4));
    }

    let address = text_input("chat.example.org", &form.address)
        .id(ADDRESS_FIELD)
        .on_input_maybe(idle.then_some(address_changed))
        .on_submit_maybe(idle.then_some(CONNECT))
        .padding([8, 10])
        .style(move |_, status| field_style(palette, status, 1.0));
    let name = text_input("", &form.name)
        .id(NAME_FIELD)
        .on_input_maybe(idle.then_some(name_changed))
        .on_submit_maybe(idle.then_some(CONNECT))
        .padding([8, 10])
        .style(move |_, status| field_style(palette, status, 1.0));

    let feedback: Element<'a, Message> = match (connecting, form.problem) {
        (Some(target), _) => paragraph(
            strings.with(
                Text::ConnectProgress,
                "address",
                &target.destination.to_string(),
            ),
            palette.muted,
        ),
        (None, Some(problem)) => paragraph(strings.text(problem.text()), palette.danger),
        (None, None) => Space::new().height(LINE_HEIGHT).into(),
    };
    let action = if idle {
        primary_button(strings.text(Text::ConnectButton), CONNECT, palette)
    } else {
        primary_button(strings.text(Text::ConnectCancelButton), CANCEL, palette)
    };

    page(
        content
            .push(label(strings.text(Text::ConnectServerLabel), palette))
            .push(address)
            .push(Space::new().height(4))
            .push(label(strings.text(Text::ConnectNameLabel), palette))
            .push(name)
            .push(feedback)
            .push(row![space::horizontal(), action]),
    )
}

fn server_row<'a>(
    strings: &Strings,
    index: usize,
    server: &'a KnownServer,
    idle: bool,
    palette: Palette,
) -> Element<'a, Message> {
    let address = server.destination().map_or_else(
        || server.address.clone(),
        |destination| destination.to_string(),
    );
    let open = Message::Servers(servers::Message::Open(index));
    let forget = Message::Servers(servers::Message::Forget(index));
    row![
        text(address).width(Fill),
        text(server.name.as_str())
            .size(13)
            .color(color(palette.muted)),
        link_button(
            strings.text(Text::ServersOpenButton),
            idle.then_some(open),
            palette
        ),
        link_button(
            strings.text(Text::ServersRemoveButton),
            idle.then_some(forget),
            palette
        ),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn chat_screen<'a>(window: &'a Window, chat: &'a Chat, palette: Palette) -> Element<'a, Message> {
    let strings = &window.strings;
    let link = |text: Text, message: chat::Message| {
        link_button(strings.text(text), Some(Message::Chat(message)), palette)
    };

    let mut links = row![].spacing(4);
    if chat.panel().is_some() {
        links = links.push(link(Text::ChatBackButton, chat::Message::Close));
    } else {
        if chat.is_owner() {
            let open = chat::Message::Open(Panel::Invitations);
            links = links.push(link(Text::ChatInvitationsButton, open));
        }
        links = links
            .push(link(
                Text::ChatNameButton,
                chat::Message::Open(Panel::Account),
            ))
            .push(link(Text::ChatLeaveButton, chat::Message::Leave));
    }
    let name = chat.session().name().as_str();
    let header = row![
        text(format!("#{LOBBY}")).font(STRONG_FONT).size(15),
        space::horizontal(),
        text(strings.with(Text::ChatStatus, "name", name))
            .size(13)
            .color(color(palette.muted)),
        links,
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .padding([8, 16]);

    let body = match chat.panel() {
        None => conversation(window, chat, palette),
        Some(Open::Invitations(panel)) => invitations_panel(window, chat, panel, palette),
        Some(Open::Account(account)) => account_panel(strings, chat, account, palette),
    };
    column![header, divider(palette), body].into()
}

fn conversation<'a>(window: &'a Window, chat: &'a Chat, palette: Palette) -> Element<'a, Message> {
    let strings = &window.strings;
    let entries = chat.session().entries();

    #[expect(
        clippy::cast_precision_loss,
        reason = "a name has a maximum of 20 characters"
    )]
    let name_width = (name_column(entries) as f32 * NAME_CHARACTER_WIDTH).max(24.0);
    let lines = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| line(window, entries, index, entry, name_width, palette));
    let timeline = scrollable(column(lines).spacing(3).padding([12, 16]))
        .anchor_bottom()
        .width(Fill)
        .height(Fill)
        .style(move |theme, status| {
            let mut style = scrollable::default(theme, status);
            style.vertical_rail.background = None;
            style.vertical_rail.scroller.background = Background::Color(color(palette.rule));
            style
        });

    let input = text_input(
        &strings.with(Text::ChatInputPlaceholder, "channel", LOBBY),
        chat.draft(),
    )
    .id(DRAFT_FIELD)
    .on_input(|draft| Message::Chat(chat::Message::DraftChanged(draft)))
    .on_submit(Message::Chat(chat::Message::Send))
    .padding([14, 16])
    .style(move |_, status| field_style(palette, status, 0.0));

    let mut page = column![timeline];
    if let Some(notice) = chat.notice() {
        page = page.push(
            container(
                text(strings.text(notice))
                    .size(13)
                    .color(color(palette.danger)),
            )
            .padding([4, 16]),
        );
    }
    page.push(divider(palette)).push(input).into()
}

fn invitations_panel<'a>(
    window: &'a Window,
    chat: &'a Chat,
    panel: &'a Invitations,
    palette: Palette,
) -> Element<'a, Message> {
    let strings = &window.strings;
    let lifetimes = Lifetime::ALL.into_iter().map(|lifetime| {
        choice(
            strings.text(lifetime.text()),
            lifetime == panel.lifetime,
            chat::Message::LifetimeSelected(lifetime),
            palette,
        )
    });
    let uses = Uses::ALL.into_iter().map(|uses| {
        let content = uses.count().map_or_else(
            || strings.text(Text::InvitationsUsesNoLimit),
            |count| count.to_string(),
        );
        let selected = chat::Message::UsesSelected(uses);
        choice(content, uses == panel.uses, selected, palette)
    });
    let notice: Element<'a, Message> = match chat.notice() {
        Some(notice) => text(strings.text(notice))
            .size(13)
            .color(color(palette.danger))
            .width(Fill)
            .into(),
        None => space::horizontal().into(),
    };
    let invite = primary_button(
        strings.text(Text::InvitationsNewButton),
        Message::Chat(chat::Message::Invite),
        palette,
    );

    let mut content = column![
        heading(strings.text(Text::InvitationsTitle)),
        label(strings.text(Text::InvitationsEndsLabel), palette),
        row(lifetimes).spacing(6),
        Space::new().height(4),
        label(strings.text(Text::InvitationsUsesLabel), palette),
        row(uses).spacing(6),
        Space::new().height(4),
        row![notice, invite].spacing(12).align_y(Alignment::Center),
        Space::new().height(4),
        divider(palette),
    ];
    match chat.session().invitations() {
        None => {
            let loading = strings.text(Text::InvitationsLoading);
            content = content.push(paragraph(loading, palette.muted));
        }
        Some([]) => {
            let empty = strings.text(Text::InvitationsEmpty);
            content = content.push(paragraph(empty, palette.muted));
        }
        Some(list) => {
            for invitation in list {
                let copied = panel.copied.as_ref() == Some(&invitation.code);
                content = content
                    .push(invitation_row(window, invitation, copied, palette))
                    .push(divider(palette));
            }
        }
    }
    panel_page(content)
}

fn invitation_row<'a>(
    window: &Window,
    invitation: &'a Invitation,
    copied: bool,
    palette: Palette,
) -> Element<'a, Message> {
    let strings = &window.strings;
    let end = invitation.end.map_or_else(
        || strings.text(Text::InvitationsLifetimeForever),
        |end| {
            let time = date_and_clock(end, &window.zone);
            strings.with(Text::InvitationsEnd, "time", &time)
        },
    );
    let count = invitation.used.to_string();
    let count = match invitation.uses {
        Some(limit) => {
            let values = [("used", count.as_str()), ("uses", &limit.to_string())];
            strings.with_all(Text::InvitationsUsedOf, &values)
        }
        None => strings.with(Text::InvitationsUsed, "used", &count),
    };
    let copy = if copied {
        Text::InvitationsCopied
    } else {
        Text::InvitationsCopyButton
    };
    let code = &invitation.code;

    column![
        text(invitation.link.as_str())
            .font(MONO_FONT)
            .size(SMALL_SIZE)
            .line_height(LineHeight::Absolute(Pixels(LINE_HEIGHT))),
        row![
            text(format!("{end} · {count}"))
                .size(13)
                .color(color(palette.muted))
                .width(Fill),
            link_button(
                strings.text(copy),
                Some(Message::Chat(chat::Message::Copy(code.clone()))),
                palette
            ),
            link_button(
                strings.text(Text::InvitationsCancelButton),
                Some(Message::Chat(chat::Message::Revoke(code.clone()))),
                palette
            ),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    ]
    .into()
}

fn account_panel<'a>(
    strings: &Strings,
    chat: &'a Chat,
    account: &'a Account,
    palette: Palette,
) -> Element<'a, Message> {
    let rename = Message::Chat(chat::Message::Rename);
    let field = text_input("", &account.name)
        .id(PANEL_FIELD)
        .on_input(|name| Message::Chat(chat::Message::NameChanged(name)))
        .on_submit(rename.clone())
        .padding([8, 10])
        .style(move |_, status| field_style(palette, status, 1.0));
    let problem = account
        .problem
        .map(|problem| servers::Problem::Name(problem).text())
        .or_else(|| chat.notice());
    let feedback: Element<'a, Message> = match problem {
        Some(problem) => paragraph(strings.text(problem), palette.danger),
        None => Space::new().height(LINE_HEIGHT).into(),
    };

    panel_page(column![
        heading(strings.text(Text::AccountTitle)),
        label(strings.text(Text::AccountNameLabel), palette),
        field,
        feedback,
        paragraph(strings.text(Text::AccountNote), palette.muted),
        Space::new().height(4),
        row![
            space::horizontal(),
            primary_button(strings.text(Text::AccountRenameButton), rename, palette)
        ],
    ])
}

fn panel_page(content: Column<'_, Message>) -> Element<'_, Message> {
    let content = content.spacing(8).max_width(PAGE_WIDTH);
    scrollable(container(content).center_x(Fill).padding(24))
        .height(Fill)
        .into()
}

fn heading<'a>(content: String) -> Element<'a, Message> {
    column![
        text(content).font(STRONG_FONT).size(18),
        Space::new().height(8)
    ]
    .into()
}

fn choice<'a>(
    content: String,
    selected: bool,
    message: chat::Message,
    palette: Palette,
) -> Element<'a, Message> {
    let font = if selected { STRONG_FONT } else { TEXT_FONT };
    button(text(content).size(13).font(font))
        .padding([4, 10])
        .on_press_maybe((!selected).then_some(Message::Chat(message)))
        .style(move |_, status| {
            let (text_color, border_color) = match status {
                button::Status::Disabled => (palette.text, palette.text),
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

fn line<'a>(
    window: &Window,
    entries: &'a [Entry],
    index: usize,
    entry: &'a Entry,
    name_width: f32,
    palette: Palette,
) -> Element<'a, Message> {
    let line_height = LineHeight::Absolute(Pixels(LINE_HEIGHT));
    let (time, name, body) = match entry {
        Entry::Message {
            time,
            from,
            text: body,
        } => (
            *time,
            shows_name(entries, index).then_some(from),
            text(body.as_str()),
        ),
        Entry::Joined { time, name } => (
            *time,
            None,
            text(window.strings.with(Text::ChatJoined, "name", name.as_str()))
                .color(color(palette.muted)),
        ),
        Entry::Left { time, name } => (
            *time,
            None,
            text(window.strings.with(Text::ChatLeft, "name", name.as_str()))
                .color(color(palette.muted)),
        ),
        Entry::Renamed { time, old, new } => {
            let values = [("old", old.as_str()), ("new", new.as_str())];
            let renamed = window.strings.with_all(Text::ChatRenamed, &values);
            (*time, None, text(renamed).color(color(palette.muted)))
        }
    };

    let stamp = text(clock(time, &window.zone))
        .font(MONO_FONT)
        .size(SMALL_SIZE)
        .line_height(line_height)
        .color(color(palette.muted));
    let name: Element<'a, Message> = match name {
        Some(name) => container(
            text(name.as_str())
                .font(STRONG_FONT)
                .size(TEXT_SIZE)
                .line_height(line_height)
                .color(color(palette.handle(name.as_str()))),
        )
        .align_right(name_width)
        .into(),
        None => Space::new().width(name_width).into(),
    };

    row![
        stamp,
        name,
        body.size(TEXT_SIZE).line_height(line_height).width(Fill),
    ]
    .spacing(10)
    .into()
}

fn label<'a>(content: String, palette: Palette) -> Element<'a, Message> {
    text(content)
        .size(SMALL_SIZE)
        .font(STRONG_FONT)
        .color(color(palette.muted))
        .into()
}

fn divider<'a>(palette: Palette) -> Element<'a, Message> {
    rule::horizontal(1)
        .style(move |theme| rule::Style {
            color: color(palette.rule),
            ..rule::default(theme)
        })
        .into()
}

fn primary_button<'a>(content: String, message: Message, palette: Palette) -> Element<'a, Message> {
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

fn link_button<'a>(
    content: String,
    message: Option<Message>,
    palette: Palette,
) -> Element<'a, Message> {
    button(text(content).size(13))
        .padding([4, 8])
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

fn field_style(
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

fn color(rgb: Rgb) -> Color {
    Color::from_rgb8(rgb.0, rgb.1, rgb.2)
}

fn mix(first: Rgb, second: Rgb, amount: f32) -> Color {
    let first = color(first);
    let second = color(second);
    Color {
        r: first.r + (second.r - first.r) * amount,
        g: first.g + (second.g - first.g) * amount,
        b: first.b + (second.b - first.b) * amount,
        a: 1.0,
    }
}

use iced::widget::text::LineHeight;
use iced::widget::{Space, column, row, space, text, text_input};
use iced::{Alignment, Element, Fill, Pixels};
use post_client::KnownServer;

use super::widgets::{
    color, field_style, label, link_button, page, page_link, paragraph, primary_button, title,
};
use super::{ADDRESS_FIELD, LINE_HEIGHT, MONO_FONT, NAME_FIELD, SMALL_SIZE, STRONG_FONT};
use crate::app::servers::{self, Question, Servers};
use crate::app::start::StartProblem;
use crate::app::{Message, Page};
use crate::i18n::{Strings, Text};
use crate::palette::Palette;

pub(super) const CONNECT: Message = Message::Servers(servers::Message::Connect);

pub(super) const CANCEL: Message = Message::Servers(servers::Message::Cancel);

pub(super) fn address_changed(address: String) -> Message {
    Message::Servers(servers::Message::AddressChanged(address))
}

pub(super) fn name_changed(name: String) -> Message {
    Message::Servers(servers::Message::NameChanged(name))
}

pub(super) fn stopped_screen<'a>(
    strings: &Strings,
    problem: &StartProblem,
    palette: Palette,
) -> Element<'a, Message> {
    page(column![
        title(),
        paragraph(problem.describe(strings), palette.danger),
    ])
}

pub(super) fn certificate_screen<'a>(
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

pub(super) fn servers_screen<'a>(
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
            .push(row![space::horizontal(), action])
            .push(Space::new().height(12))
            .push(
                row![
                    page_link(strings, Page::Profiles, [4, 0], palette),
                    page_link(strings, Page::Settings, [4, 0], palette),
                ]
                .spacing(16),
            ),
    )
}

pub(super) fn server_row<'a>(
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

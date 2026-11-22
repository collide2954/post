use iced::widget::text::LineHeight;
use iced::widget::{
    Space, column, container, responsive, row, scrollable, space, text, text_input,
};
use iced::{Alignment, Background, Element, Fill, Pixels};
use post_client::Entry;
use post_proto::{Invitation, LOBBY, Timestamp};

use super::widgets::{
    bar, choice, color, divider, field_style, heading, label, link_button, panel_page, paragraph,
    primary_button, settings_link,
};
use super::{
    BAR_HEIGHT, DRAFT_FIELD, LINE_HEIGHT, MONO_FONT, NAME_CHARACTER_WIDTH, PANEL_FIELD, SMALL_SIZE,
    STATUS_WIDTH, STRONG_FONT, TEXT_SIZE, Window,
};
use crate::app::chat::{self, Account, Chat, Invitations, Lifetime, Open, Panel, Uses};
use crate::app::{Message, servers};
use crate::i18n::{Strings, Text};
use crate::palette::Palette;
use crate::timeline::{ClockStyle, clock, date_and_clock, name_column, shows_name};

pub(super) fn chat_screen<'a>(
    window: &'a Window,
    chat: &'a Chat,
    palette: Palette,
) -> Element<'a, Message> {
    let strings = &window.strings;
    let header =
        responsive(move |size| chat_bar(strings, chat, size.width >= STATUS_WIDTH, palette));
    let body = match chat.panel() {
        None => conversation(window, chat, palette),
        Some(Open::Invitations(panel)) => invitations_panel(window, chat, panel, palette),
        Some(Open::Account(account)) => account_panel(strings, chat, account, palette),
    };
    column![container(header).height(BAR_HEIGHT), body].into()
}

pub(super) fn chat_bar<'a>(
    strings: &Strings,
    chat: &Chat,
    with_status: bool,
    palette: Palette,
) -> Element<'a, Message> {
    let link = |text: Text, message: chat::Message| {
        link_button(strings.text(text), Some(Message::Chat(message)), palette)
    };

    let mut links = row![].spacing(4);
    if chat.panel().is_some() {
        links = links.push(link(Text::NavBackButton, chat::Message::Close));
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
            .push(settings_link(strings, [4, 8], palette))
            .push(link(Text::ChatLeaveButton, chat::Message::Leave));
    }
    let name = chat.session().name().as_str();
    let status = with_status.then(|| strings.with(Text::ChatStatus, "name", name));
    bar(format!("#{LOBBY}"), status, links, palette)
}

pub(super) fn conversation<'a>(
    window: &'a Window,
    chat: &'a Chat,
    palette: Palette,
) -> Element<'a, Message> {
    let strings = &window.strings;
    let entries = chat.session().entries();

    #[expect(
        clippy::cast_precision_loss,
        reason = "a name has a maximum of 20 characters"
    )]
    let name_width = (name_column(entries) as f32 * NAME_CHARACTER_WIDTH).max(24.0);
    let style = clock_style(window);
    let lines = entries.iter().enumerate().map(|(index, entry)| {
        let stamp = clock(entry_time(entry), &window.zone, &style);
        line(strings, stamp, entries, index, entry, name_width, palette)
    });
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

pub(super) fn invitations_panel<'a>(
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
            Message::Chat(chat::Message::LifetimeSelected(lifetime)),
            palette,
        )
    });
    let uses = Uses::ALL.into_iter().map(|uses| {
        let content = uses.count().map_or_else(
            || strings.text(Text::InvitationsUsesNoLimit),
            |count| count.to_string(),
        );
        let select = Message::Chat(chat::Message::UsesSelected(uses));
        choice(content, uses == panel.uses, select, palette)
    });
    let style = clock_style(window);
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
                let end = invitation
                    .end
                    .map(|end| date_and_clock(end, &window.zone, &style));
                content = content
                    .push(invitation_row(strings, invitation, end, copied, palette))
                    .push(divider(palette));
            }
        }
    }
    panel_page(content)
}

pub(super) fn invitation_row<'a>(
    strings: &Strings,
    invitation: &'a Invitation,
    end: Option<String>,
    copied: bool,
    palette: Palette,
) -> Element<'a, Message> {
    let end = end.map_or_else(
        || strings.text(Text::InvitationsLifetimeForever),
        |time| strings.with(Text::InvitationsEnd, "time", &time),
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

pub(super) fn account_panel<'a>(
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

pub(super) fn clock_style(window: &Window) -> ClockStyle {
    ClockStyle::new(
        window.app.settings(),
        window.strings.text(Text::ClockMorning),
        window.strings.text(Text::ClockAfternoon),
    )
}

pub(super) fn entry_time(entry: &Entry) -> Timestamp {
    match entry {
        Entry::Message { time, .. }
        | Entry::Joined { time, .. }
        | Entry::Left { time, .. }
        | Entry::Renamed { time, .. } => *time,
    }
}

pub(super) fn line<'a>(
    strings: &Strings,
    stamp: String,
    entries: &'a [Entry],
    index: usize,
    entry: &'a Entry,
    name_width: f32,
    palette: Palette,
) -> Element<'a, Message> {
    let line_height = LineHeight::Absolute(Pixels(LINE_HEIGHT));
    let (name, body) = match entry {
        Entry::Message {
            from, text: body, ..
        } => (
            shows_name(entries, index).then_some(from),
            text(body.as_str()),
        ),
        Entry::Joined { name, .. } => (
            None,
            text(strings.with(Text::ChatJoined, "name", name.as_str())).color(color(palette.muted)),
        ),
        Entry::Left { name, .. } => (
            None,
            text(strings.with(Text::ChatLeft, "name", name.as_str())).color(color(palette.muted)),
        ),
        Entry::Renamed { old, new, .. } => {
            let values = [("old", old.as_str()), ("new", new.as_str())];
            let renamed = strings.with_all(Text::ChatRenamed, &values);
            (None, text(renamed).color(color(palette.muted)))
        }
    };

    let stamp = text(stamp)
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

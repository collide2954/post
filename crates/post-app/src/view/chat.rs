use iced::widget::text::LineHeight;
use iced::widget::{
    Column, Space, column, container, responsive, row, scrollable, space, text, text_input,
};
use iced::{Alignment, Background, Element, Fill, Pixels};
use post_client::Entry;
use post_proto::{ChannelKind, ChannelName, Invitation, ListedChannel, Timestamp};

use super::widgets::{
    bar, choice, color, divider, entry, field_style, heading, label, link, link_button, page_link,
    panel_page, paragraph, primary_button, vertical_divider,
};
use super::{
    BAR_HEIGHT, DRAFT_FIELD, LINE_HEIGHT, MONO_FONT, NAME_CHARACTER_WIDTH, PANEL_FIELD,
    SIDEBAR_WIDTH, SMALL_SIZE, STRONG_FONT, TEXT_SIZE, WIDE_WIDTH, Window,
};
use crate::app::chat::{self, Account, Channels, Chat, Invitations, Lifetime, Open, Panel, Uses};
use crate::app::{Message, Page, servers};
use crate::i18n::{Strings, Text};
use crate::palette::Palette;
use crate::timeline::{ClockStyle, clock, date_and_clock, name_column, shows_name};

pub(super) fn chat_screen<'a>(
    window: &'a Window,
    chat: &'a Chat,
    palette: Palette,
) -> Element<'a, Message> {
    let strings = &window.strings;
    responsive(move |size| {
        let is_wide = size.width >= WIDE_WIDTH;
        let body = match chat.panel() {
            None => conversation(window, chat, palette),
            Some(Open::Invitations(panel)) => invitations_panel(window, chat, panel, palette),
            Some(Open::Account(account)) => account_panel(strings, chat, account, palette),
            Some(Open::Menu) => panel_page(navigation(strings, chat, palette)),
            Some(Open::Channels(panel)) => channels_panel(strings, chat, panel, palette),
        };
        let header = chat_bar(strings, chat, is_wide, palette);
        let main = column![container(header).height(BAR_HEIGHT), body];
        if is_wide {
            let sidebar = scrollable(navigation(strings, chat, palette).padding([8, 0]))
                .width(SIDEBAR_WIDTH)
                .height(Fill);
            row![sidebar, vertical_divider(palette), main].into()
        } else {
            main.into()
        }
    })
    .into()
}

pub(super) fn chat_bar<'a>(
    strings: &Strings,
    chat: &Chat,
    is_wide: bool,
    palette: Palette,
) -> Element<'a, Message> {
    let link = |text: Text, message: chat::Message| {
        link_button(strings.text(text), Some(Message::Chat(message)), palette)
    };

    let mut links = row![].spacing(4);
    if chat.panel().is_some() {
        links = links.push(link(Text::NavBackButton, chat::Message::Close));
    } else {
        if chat.place().is_some() {
            let leave = chat::Message::LeaveChannel;
            links = links.push(link(Text::ChatLeaveChannelButton, leave));
        }
        if !is_wide {
            let open = chat::Message::Open(Panel::Menu);
            links = links.push(link(Text::ChatMenuButton, open));
        }
    }
    let title = chat.place().map_or_else(
        || chat.target().destination.address().to_string(),
        |place| format!("#{place}"),
    );
    bar(title, None, links, palette)
}

pub(super) fn navigation<'a>(
    strings: &Strings,
    chat: &Chat,
    palette: Palette,
) -> Column<'a, Message> {
    let address = chat.target().destination.address().to_string();
    let status = strings.with(Text::ChatStatus, "name", chat.session().name().as_str());
    let place = |name: &ChannelName| {
        let selected = chat.place() == Some(name);
        let select = Message::Chat(chat::Message::Select(name.clone()));
        entry(format!("#{name}"), selected, select, palette)
    };
    let action = |text: Text, message: chat::Message| {
        let message = Some(Message::Chat(message));
        link(strings.text(text), message, [5, 16], palette)
    };

    let places = Column::with_children(chat.session().channels().map(|(name, _)| place(name)));
    let mut actions = column![action(
        Text::ChatChannelsButton,
        chat::Message::Open(Panel::Channels)
    )];
    if chat.is_owner() {
        let open = chat::Message::Open(Panel::Invitations);
        actions = actions.push(action(Text::ChatInvitationsButton, open));
    }
    actions = actions
        .push(action(
            Text::ChatNameButton,
            chat::Message::Open(Panel::Account),
        ))
        .push(page_link(strings, Page::Settings, [5, 16], palette))
        .push(action(Text::ChatLeaveButton, chat::Message::Leave));

    column![
        container(
            column![
                text(address).font(STRONG_FONT).size(13),
                text(status).size(SMALL_SIZE).color(color(palette.muted)),
            ]
            .spacing(2)
        )
        .padding([8, 16]),
        places,
        container(divider(palette)).padding([8, 16]),
        actions,
    ]
}

pub(super) fn conversation<'a>(
    window: &'a Window,
    chat: &'a Chat,
    palette: Palette,
) -> Element<'a, Message> {
    let strings = &window.strings;
    let Some(place) = chat.place() else {
        return no_channel(strings, chat, palette);
    };
    let entries = chat.entries();

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
        &strings.with(Text::ChatInputPlaceholder, "channel", place.as_str()),
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

fn no_channel<'a>(strings: &Strings, chat: &Chat, palette: Palette) -> Element<'a, Message> {
    if !chat.session().is_ready() {
        return space().width(Fill).height(Fill).into();
    }
    let open = Message::Chat(chat::Message::Open(Panel::Channels));
    let button = primary_button(strings.text(Text::ChatChannelsButton), open, palette);
    let mut content = column![
        heading(strings.text(Text::ChatNoChannelTitle)),
        paragraph(strings.text(Text::ChatNoChannelNote), palette.muted),
        Space::new().height(4),
        row![button],
    ];
    if let Some(notice) = chat.notice() {
        content = content.push(paragraph(strings.text(notice), palette.danger));
    }
    panel_page(content)
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

pub(super) fn channels_panel<'a>(
    strings: &Strings,
    chat: &'a Chat,
    panel: &'a Channels,
    palette: Palette,
) -> Element<'a, Message> {
    let make = Message::Chat(chat::Message::MakeChannel);
    let field = text_input("", &panel.name)
        .id(PANEL_FIELD)
        .on_input(|name| Message::Chat(chat::Message::ChannelNameChanged(name)))
        .on_submit(make.clone())
        .padding([8, 10])
        .style(move |_, status| field_style(palette, status, 1.0));
    let kinds = [
        (ChannelKind::Public, Text::ChannelsKindPublic),
        (ChannelKind::Private, Text::ChannelsKindPrivate),
    ]
    .into_iter()
    .map(|(kind, text)| {
        let select = Message::Chat(chat::Message::KindSelected(kind));
        choice(strings.text(text), kind == panel.kind, select, palette)
    });
    let feedback: Element<'a, Message> = match panel.problem_text().or_else(|| chat.notice()) {
        Some(problem) => paragraph(strings.text(problem), palette.danger),
        None => Space::new().height(LINE_HEIGHT).into(),
    };
    let button = primary_button(strings.text(Text::ChannelsMakeButton), make, palette);

    let mut content = column![
        heading(strings.text(Text::ChannelsTitle)),
        label(strings.text(Text::ChannelsNameLabel), palette),
        field,
        Space::new().height(4),
        row![row(kinds).spacing(6), space::horizontal(), button].align_y(Alignment::Center),
        feedback,
        divider(palette),
    ];
    match chat.session().listed() {
        None => {
            let loading = strings.text(Text::ChannelsLoading);
            content = content.push(paragraph(loading, palette.muted));
        }
        Some([]) => {
            let empty = strings.text(Text::ChannelsEmpty);
            content = content.push(paragraph(empty, palette.muted));
        }
        Some(list) => {
            for listed in list {
                content = content
                    .push(channel_row(strings, chat, listed, palette))
                    .push(divider(palette));
            }
        }
    }
    panel_page(content)
}

pub(super) fn channel_row<'a>(
    strings: &Strings,
    chat: &Chat,
    listed: &'a ListedChannel,
    palette: Palette,
) -> Element<'a, Message> {
    let members = if listed.members == 1 {
        strings.text(Text::ChannelsMember)
    } else {
        strings.with(Text::ChannelsMembers, "count", &listed.members.to_string())
    };
    let detail = if listed.topic.is_empty() {
        members
    } else {
        format!("{} · {members}", listed.topic)
    };
    let name = listed.channel.clone();
    let (action, message) = if chat.session().channel(&name).is_some() {
        (Text::ChannelsOpenButton, chat::Message::Select(name))
    } else {
        (Text::ChannelsJoinButton, chat::Message::Join(name))
    };

    column![
        row![
            text(format!("#{}", listed.channel))
                .font(STRONG_FONT)
                .width(Fill),
            link_button(strings.text(action), Some(Message::Chat(message)), palette),
        ]
        .align_y(Alignment::Center),
        text(detail).size(13).color(color(palette.muted)),
    ]
    .into()
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

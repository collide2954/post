use iced::widget::{Column, Space, column, row, text, text_input};
use iced::{Alignment, Element, Fill};
use post_client::ProfileEntry;

use super::widgets::{
    bar, color, danger_button, field_style, label, link_button, panel_page, paragraph,
    primary_button,
};
use super::{LINE_HEIGHT, PANEL_FIELD, PROFILE_FIELD, STRONG_FONT, Window};
use crate::app::Message;
use crate::app::profiles::{self, Edit, Profiles};
use crate::i18n::{Strings, Text};
use crate::palette::Palette;

const CONFIRM: Message = Message::Profiles(profiles::Message::Confirm);
const CANCEL: Message = Message::Profiles(profiles::Message::Cancel);
const CREATE: Message = Message::Profiles(profiles::Message::Create);

fn edit_changed(text: String) -> Message {
    Message::Profiles(profiles::Message::EditChanged(text))
}

pub(super) fn profiles_page(window: &Window, palette: Palette) -> Element<'_, Message> {
    let strings = &window.strings;
    let profiles = window.app.profiles();
    let back = link_button(
        strings.text(Text::NavBackButton),
        Some(Message::Back),
        palette,
    );

    let mut content = column![
        paragraph(strings.text(Text::ProfilesNote), palette.muted),
        Space::new().height(4),
    ];
    for profile in profiles.list() {
        content = content.push(match profiles.edit() {
            Some(Edit::Rename { id, name }) if *id == profile.id => {
                rename_row(strings, profiles, name, palette)
            }
            Some(Edit::Delete { id, typed }) if *id == profile.id => {
                delete_question(strings, profiles, profile, typed, palette)
            }
            _ => profile_row(strings, profiles, profile, palette),
        });
    }

    let idle = profiles.edit().is_none();
    let name = text_input("", profiles.new_name())
        .id(PROFILE_FIELD)
        .on_input_maybe(
            idle.then_some(|name| Message::Profiles(profiles::Message::NewNameChanged(name))),
        )
        .on_submit_maybe(idle.then_some(CREATE))
        .padding([8, 10])
        .style(move |_, status| field_style(palette, status, 1.0));
    let create = primary_button(strings.text(Text::ProfilesCreateButton), CREATE, palette);
    let content = content
        .push(Space::new().height(12))
        .push(label(strings.text(Text::ProfilesNewLabel), palette))
        .push(row![name, create].spacing(8).align_y(Alignment::Center));
    let content = if idle {
        content.push(feedback(strings, profiles, palette))
    } else {
        content
    };

    column![
        bar(strings.text(Text::ProfilesTitle), None, row![back], palette),
        panel_page(content),
    ]
    .into()
}

fn profile_row<'a>(
    strings: &Strings,
    profiles: &Profiles,
    profile: &'a ProfileEntry,
    palette: Palette,
) -> Element<'a, Message> {
    let idle = profiles.edit().is_none();
    let send = |message: profiles::Message| idle.then_some(Message::Profiles(message));
    let id = profile.id;
    let mut line = row![text(profile.name.as_str()).width(Fill)]
        .spacing(4)
        .align_y(Alignment::Center);
    if profiles.current() == Some(id) {
        line = line
            .push(
                text(strings.text(Text::ProfilesCurrent))
                    .size(13)
                    .color(color(palette.muted)),
            )
            .push(Space::new().width(8))
            .push(link_button(
                strings.text(Text::ProfilesRenameButton),
                send(profiles::Message::Rename(id)),
                palette,
            ));
    } else {
        line = line
            .push(link_button(
                strings.text(Text::ProfilesOpenButton),
                send(profiles::Message::Switch(id)),
                palette,
            ))
            .push(link_button(
                strings.text(Text::ProfilesRenameButton),
                send(profiles::Message::Rename(id)),
                palette,
            ))
            .push(link_button(
                strings.text(Text::ProfilesDeleteButton),
                send(profiles::Message::Delete(id)),
                palette,
            ));
    }
    line.into()
}

fn rename_row<'a>(
    strings: &Strings,
    profiles: &Profiles,
    name: &str,
    palette: Palette,
) -> Element<'a, Message> {
    let field = text_input("", name)
        .id(PANEL_FIELD)
        .on_input(edit_changed)
        .on_submit(CONFIRM)
        .padding([8, 10])
        .style(move |_, status| field_style(palette, status, 1.0));
    let buttons = row![
        field,
        link_button(
            strings.text(Text::ProfilesSaveButton),
            Some(CONFIRM),
            palette
        ),
        link_button(
            strings.text(Text::ConnectCancelButton),
            Some(CANCEL),
            palette
        ),
    ]
    .spacing(4)
    .align_y(Alignment::Center);
    edit_block(column![buttons], strings, profiles, palette)
}

fn delete_question<'a>(
    strings: &Strings,
    profiles: &Profiles,
    profile: &ProfileEntry,
    typed: &str,
    palette: Palette,
) -> Element<'a, Message> {
    let warning = strings.with(Text::ProfilesDeleteWarning, "name", &profile.name);
    let field = text_input("", typed)
        .id(PANEL_FIELD)
        .on_input(edit_changed)
        .on_submit(CONFIRM)
        .padding([8, 10])
        .style(move |_, status| field_style(palette, status, 1.0));
    let delete = danger_button(
        strings.text(Text::ProfilesDeleteConfirmButton),
        profiles.can_delete().then_some(CONFIRM),
        palette,
    );
    let cancel = link_button(
        strings.text(Text::ConnectCancelButton),
        Some(CANCEL),
        palette,
    );
    let block = column![
        text(profile.name.clone()).font(STRONG_FONT),
        paragraph(warning, palette.danger),
        label(strings.text(Text::ProfilesDeleteConfirmLabel), palette),
        field,
        row![Space::new().width(Fill), cancel, delete]
            .spacing(8)
            .align_y(Alignment::Center),
    ];
    edit_block(block, strings, profiles, palette)
}

fn edit_block<'a>(
    block: Column<'a, Message>,
    strings: &Strings,
    profiles: &Profiles,
    palette: Palette,
) -> Element<'a, Message> {
    block
        .push(feedback(strings, profiles, palette))
        .spacing(8)
        .into()
}

fn feedback<'a>(strings: &Strings, profiles: &Profiles, palette: Palette) -> Element<'a, Message> {
    match profiles.problem() {
        Some(problem) => paragraph(strings.text(problem.text()), palette.danger),
        None => Space::new().height(LINE_HEIGHT).into(),
    }
}

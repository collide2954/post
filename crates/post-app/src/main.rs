#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use post_app::arguments::Arguments;
use post_app::i18n::Language;
use post_app::shell;
use post_client::Home;

const LANGUAGE_VARIABLE: &str = "POST_LANGUAGE";

fn main() -> iced::Result {
    let language = std::env::var(LANGUAGE_VARIABLE)
        .ok()
        .and_then(|tag| Language::from_tag(&tag))
        .unwrap_or_else(|| Language::from_locale(sys_locale::get_locale().as_deref()));
    let arguments = std::env::args_os()
        .skip(1)
        .map(|argument| argument.to_string_lossy().into_owned());
    let arguments = Arguments::parse(arguments);
    shell::application(language, Home::of_this_user(), arguments).run()
}

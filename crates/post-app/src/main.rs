#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use post_app::i18n::Language;
use post_app::shell;

const LANGUAGE_VARIABLE: &str = "POST_LANGUAGE";

fn main() -> iced::Result {
    let language = std::env::var(LANGUAGE_VARIABLE)
        .ok()
        .and_then(|tag| Language::from_tag(&tag))
        .unwrap_or_else(|| Language::from_locale(sys_locale::get_locale().as_deref()));
    shell::application(language).run()
}

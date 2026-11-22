use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::Duration;

use iced::futures::Stream;
use iced::futures::stream;
use iced::keyboard::{self, Key, key::Named};
use iced::widget::operation;
use iced::{
    Application, Element, Event, Pixels, Program, Size, Subscription, Task, Theme, event, theme,
    window,
};
use jiff::tz::TimeZone;
use post_client::{Home, Identity, Profile, ProfileError, ProfileId, Profiles, Seed};

use crate::app::servers::Target;
use crate::app::start::StartProblem;
use crate::app::{self, App, Effect, Message};
use crate::arguments::{ArgumentError, Arguments};
use crate::i18n::{Language, Strings, Text};
use crate::palette::Mode;
use crate::settings::{Settings, TEMPLATE};
use crate::view::{self, Window};

const WATCH_INTERVAL: Duration = Duration::from_secs(1);

pub struct Shell {
    window: Window,
    profiles: Option<Profiles>,
    profile: Option<Profile>,
    system_language: Language,
}

#[derive(Hash)]
struct Watch {
    file: PathBuf,
}

struct SignIn {
    target: Target,
    seed: Seed,
}

impl Hash for SignIn {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.target.attempt.hash(state);
    }
}

pub fn application(
    language: Language,
    home: Option<Home>,
    arguments: Result<Arguments, ArgumentError>,
) -> Application<impl Program<State = Shell, Message = Message, Theme = Theme>> {
    let boot = move || boot(language, home.clone(), arguments.clone());
    let mut application = iced::application(boot, update, view)
        .title("Post")
        .theme(window_theme)
        .subscription(subscription)
        .scale_factor(|shell| shell.window.app.settings().scale())
        .settings(iced::Settings {
            default_text_size: Pixels(14.0),
            ..iced::Settings::default()
        })
        .default_font(view::TEXT_FONT)
        .window(window::Settings {
            size: Size::new(900.0, 600.0),
            min_size: Some(Size::new(520.0, 320.0)),
            ..window::Settings::default()
        });
    for font in view::FONTS {
        application = application.font(font);
    }
    application
}

fn boot(
    language: Language,
    home: Option<Home>,
    arguments: Result<Arguments, ArgumentError>,
) -> (Shell, Task<Message>) {
    let strings = Strings::new(language);
    let profiles = home.map(Profiles::new);
    let first_name = strings.text(Text::ProfileFirstName);
    let profile = open_at_start(profiles.as_ref(), arguments, &first_name);
    let mut shell = Shell {
        window: Window {
            app: App::stopped(Mode::Light, StartProblem::NoHome),
            strings,
            zone: TimeZone::system(),
            settings_file: None,
        },
        profiles,
        profile: None,
        system_language: language,
    };
    match profile {
        Ok(profile) => {
            let app = App::new(Mode::Light, profile.servers().to_vec());
            shell.enter(app, profile);
        }
        Err(problem) => shell.window.app = App::stopped(Mode::Light, problem),
    }
    let tasks = Task::batch([
        iced::system::theme().map(mode_changed),
        operation::focus(view::ADDRESS_FIELD),
    ]);
    (shell, tasks)
}

fn open_at_start(
    profiles: Option<&Profiles>,
    arguments: Result<Arguments, ArgumentError>,
    first_name: &str,
) -> Result<Profile, StartProblem> {
    let arguments = arguments?;
    let profiles = profiles.ok_or(StartProblem::NoHome)?;
    match arguments.profile {
        Some(name) => profiles.open_named(&name).map_err(|error| match error {
            ProfileError::NotFound => StartProblem::NoProfile(name),
            other => other.into(),
        }),
        None => Ok(profiles.start(first_name)?),
    }
}

impl Shell {
    fn enter(&mut self, mut app: App, profile: Profile) {
        match profile.read_settings() {
            Ok(Some(text)) => {
                app.update(Message::Settings(app::settings::Message::FileChanged(text)));
            }
            Ok(None) => {
                let _ = profile.write_settings(TEMPLATE);
            }
            Err(_) => {}
        }
        self.window.settings_file = Some(profile.settings_file().display().to_string());
        self.window.app = app;
        self.profile = Some(profile);
        self.obey_the_language();
    }

    fn obey_the_language(&mut self) {
        let settings = self.window.app.settings();
        let language = settings.language.unwrap_or(self.system_language);
        if language != self.window.strings.language() {
            self.window.strings = Strings::new(language);
        }
    }

    fn list_profiles(&mut self) {
        let (Some(profiles), Some(profile)) = (&self.profiles, &self.profile) else {
            return;
        };
        let message = match profiles.list() {
            Ok(list) => app::profiles::Message::Listed(list, profile.id()),
            Err(error) => app::profiles::Message::Failed((&error).into()),
        };
        self.window.app.update(Message::Profiles(message));
    }

    fn change_profiles(&mut self, change: impl FnOnce(&Profiles) -> Result<(), ProfileError>) {
        let Some(profiles) = &self.profiles else {
            return;
        };
        match change(profiles) {
            Ok(()) => self.list_profiles(),
            Err(error) => self.fail(&error),
        }
    }

    fn switch(&mut self, id: ProfileId) -> Task<Message> {
        let Some(profiles) = self.profiles.clone() else {
            return Task::none();
        };
        match profiles.open(id) {
            Ok(profile) => {
                let servers = profile.servers().to_vec();
                let app = self.window.app.for_a_different_profile(servers);
                self.enter(app, profile);
                operation::focus(view::ADDRESS_FIELD)
            }
            Err(error) => {
                self.fail(&error);
                Task::none()
            }
        }
    }

    fn fail(&mut self, error: &ProfileError) {
        let failed = app::profiles::Message::Failed(error.into());
        self.window.app.update(Message::Profiles(failed));
    }
}

fn update(shell: &mut Shell, message: Message) -> Task<Message> {
    let effect = shell.window.app.update(message);
    shell.obey_the_language();
    match effect {
        Effect::None => Task::none(),
        Effect::FocusForm => operation::focus(view::ADDRESS_FIELD),
        Effect::FocusDraft => operation::focus(view::DRAFT_FIELD),
        Effect::FocusPanel => operation::focus(view::PANEL_FIELD),
        Effect::FocusNext => operation::focus_next(),
        Effect::FocusPrevious => operation::focus_previous(),
        Effect::Copy(text) => iced::clipboard::write(text),
        Effect::Remember(server) => {
            let saved = shell
                .profile
                .as_mut()
                .is_some_and(|profile| profile.remember(server).is_ok());
            if !saved {
                shell.window.app.update(Message::ProfileNotSaved);
            }
            operation::focus(view::DRAFT_FIELD)
        }
        Effect::Forget(address) => {
            let saved = shell
                .profile
                .as_mut()
                .is_some_and(|profile| profile.forget(&address).is_ok());
            if !saved {
                shell.window.app.update(Message::ProfileNotSaved);
            }
            Task::none()
        }
        Effect::SaveSettings(settings) => {
            let saved = shell
                .profile
                .as_ref()
                .is_some_and(|profile| save(profile, settings).is_ok());
            if !saved {
                let not_saved = app::settings::Message::NotSaved;
                shell.window.app.update(Message::Settings(not_saved));
            }
            Task::none()
        }
        Effect::ListProfiles => {
            shell.list_profiles();
            Task::none()
        }
        Effect::CreateProfile(name) => {
            shell.change_profiles(|profiles| profiles.create(&name).map(drop));
            Task::none()
        }
        Effect::RenameProfile(id, name) => {
            shell.change_profiles(|profiles| profiles.rename(id, &name));
            Task::none()
        }
        Effect::DeleteProfile(id) => {
            shell.change_profiles(|profiles| profiles.delete(id));
            Task::none()
        }
        Effect::SwitchProfile(id) => shell.switch(id),
    }
}
fn save(profile: &Profile, settings: Settings) -> Result<(), ProfileError> {
    let text = profile.read_settings()?;
    let text = settings.write_into(text.as_deref().unwrap_or(TEMPLATE));
    profile.write_settings(&text)
}

fn watch(file: PathBuf, interval: Duration) -> impl Stream<Item = String> {
    let seen = std::fs::read_to_string(&file).ok();
    stream::unfold((file, seen), move |(file, seen)| async move {
        loop {
            tokio::time::sleep(interval).await;
            if let Ok(text) = std::fs::read_to_string(&file)
                && seen.as_ref() != Some(&text)
            {
                return Some((text.clone(), (file, Some(text))));
            }
        }
    })
}

fn view(shell: &Shell) -> Element<'_, Message> {
    view::view(&shell.window)
}

fn window_theme(shell: &Shell) -> Theme {
    view::theme(shell.window.app.mode())
}

fn subscription(shell: &Shell) -> Subscription<Message> {
    let connection = match (shell.window.app.target(), &shell.profile) {
        (Some(target), Some(profile)) => {
            let sign_in = SignIn {
                target: target.clone(),
                seed: profile.seed().clone(),
            };
            Subscription::run_with(sign_in, |sign_in| {
                let identity = Identity {
                    seed: sign_in.seed.clone(),
                    name: sign_in.target.name.clone(),
                };
                post_client::sign_in(
                    sign_in.target.destination.clone(),
                    identity,
                    sign_in.target.trust,
                )
            })
            .with(target.attempt)
            .map(|(attempt, event)| Message::Connection(attempt, event))
        }
        _ => Subscription::none(),
    };
    let settings = match &shell.profile {
        Some(profile) => {
            let file = Watch {
                file: profile.settings_file().to_owned(),
            };
            Subscription::run_with(file, |watched| watch(watched.file.clone(), WATCH_INTERVAL))
                .map(|text| Message::Settings(app::settings::Message::FileChanged(text)))
        }
        None => Subscription::none(),
    };
    Subscription::batch([
        connection,
        settings,
        iced::system::theme_changes().map(mode_changed),
        event::listen_with(|event, status, _window| shortcut(&event, status)),
    ])
}

fn mode_changed(mode: theme::Mode) -> Message {
    Message::ModeChanged(match mode {
        theme::Mode::Dark => Mode::Dark,
        theme::Mode::Light | theme::Mode::None => Mode::Light,
    })
}

fn shortcut(event: &Event, status: event::Status) -> Option<Message> {
    let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
        return None;
    };
    let size = |message| Some(Message::Settings(message));
    match (key.as_ref(), status) {
        (Key::Named(Named::Escape), _) => Some(Message::Back),
        (Key::Named(Named::Tab), event::Status::Ignored) => Some(if modifiers.shift() {
            Message::FocusPrevious
        } else {
            Message::FocusNext
        }),
        (Key::Character("+" | "="), _) if modifiers.command() => {
            size(app::settings::Message::Larger)
        }
        (Key::Character("-"), _) if modifiers.command() => size(app::settings::Message::Smaller),
        (Key::Character("0"), _) if modifiers.command() => {
            size(app::settings::Message::DefaultSize)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use iced::event::Status;
    use iced::keyboard::key::{self, Named};
    use iced::keyboard::{self, Key, Location, Modifiers};
    use iced::{Event, mouse, theme};

    use std::time::Duration;

    use iced::futures::{Stream, StreamExt};
    use post_client::{Home, Profiles};
    use post_testing::Directory;

    use super::{mode_changed, open_at_start, save, shortcut, watch};
    use crate::app::start::StartProblem;
    use crate::app::{Message, settings};
    use crate::arguments::{ArgumentError, Arguments};
    use crate::palette::Mode;
    use crate::settings::{Settings, TEMPLATE};

    fn pressed(key: Key, modifiers: Modifiers) -> Event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: key::Physical::Unidentified(key::NativeCode::Unidentified),
            location: Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        })
    }

    #[test]
    fn the_tab_key_moves_the_focus_to_the_next_field() {
        let event = pressed(Key::Named(Named::Tab), Modifiers::empty());

        assert!(matches!(
            shortcut(&event, Status::Ignored),
            Some(Message::FocusNext)
        ));
    }

    #[test]
    fn the_shift_and_tab_keys_move_the_focus_to_the_field_before() {
        let event = pressed(Key::Named(Named::Tab), Modifiers::SHIFT);

        assert!(matches!(
            shortcut(&event, Status::Ignored),
            Some(Message::FocusPrevious)
        ));
    }

    #[test]
    fn the_tab_key_does_nothing_when_a_part_of_the_window_used_it() {
        let event = pressed(Key::Named(Named::Tab), Modifiers::empty());

        assert!(shortcut(&event, Status::Captured).is_none());
    }

    #[test]
    fn the_escape_key_goes_back_also_from_a_text_field() {
        let event = pressed(Key::Named(Named::Escape), Modifiers::empty());

        for status in [Status::Ignored, Status::Captured] {
            assert!(
                matches!(shortcut(&event, status), Some(Message::Back)),
                "{status:?}"
            );
        }
    }

    #[test]
    fn the_command_key_with_plus_minus_and_zero_changes_the_text_size() {
        let character = |text: &str| pressed(Key::Character(text.into()), Modifiers::COMMAND);

        for status in [Status::Ignored, Status::Captured] {
            for larger in ["+", "="] {
                assert!(matches!(
                    shortcut(&character(larger), status),
                    Some(Message::Settings(settings::Message::Larger))
                ));
            }
            assert!(matches!(
                shortcut(&character("-"), status),
                Some(Message::Settings(settings::Message::Smaller))
            ));
            assert!(matches!(
                shortcut(&character("0"), status),
                Some(Message::Settings(settings::Message::DefaultSize))
            ));
        }
        let plain = pressed(Key::Character("+".into()), Modifiers::empty());
        assert!(shortcut(&plain, Status::Ignored).is_none());
        let other = pressed(Key::Character("a".into()), Modifiers::COMMAND);
        assert!(shortcut(&other, Status::Ignored).is_none());
    }

    fn named(name: &str) -> Arguments {
        Arguments {
            profile: Some(name.to_owned()),
        }
    }

    #[test]
    fn opens_the_profile_that_the_command_line_names() {
        let directory = Directory::new();
        let profiles = Profiles::new(Home::at(directory.path()));
        profiles.create("main").expect("a profile");
        profiles.create("work").expect("a profile");

        let profile = open_at_start(Some(&profiles), Ok(named("Work")), "main").expect("a profile");
        assert_eq!(profile.name(), "work");
        drop(profile);

        let problem = open_at_start(Some(&profiles), Ok(named("garden")), "main").err();
        assert_eq!(problem, Some(StartProblem::NoProfile("garden".to_owned())));
        assert_eq!(profiles.list().expect("a list").len(), 2);
    }

    #[test]
    fn opens_the_first_profile_when_the_command_line_names_none() {
        let directory = Directory::new();
        let profiles = Profiles::new(Home::at(directory.path()));

        let profile = open_at_start(Some(&profiles), Ok(Arguments::default()), "principal")
            .expect("a profile");

        assert_eq!(profile.name(), "principal");
    }

    #[test]
    fn opens_no_profile_when_the_start_has_a_problem() {
        let directory = Directory::new();
        let profiles = Profiles::new(Home::at(directory.path()));
        let unknown = Err(ArgumentError::Unknown("--fast".to_owned()));

        assert_eq!(
            open_at_start(Some(&profiles), unknown, "main").err(),
            Some(StartProblem::Argument("--fast".to_owned()))
        );
        assert_eq!(
            open_at_start(None, Err(ArgumentError::NoProfileName), "main").err(),
            Some(StartProblem::NoProfileName)
        );
        assert_eq!(
            open_at_start(None, Ok(Arguments::default()), "main").err(),
            Some(StartProblem::NoHome)
        );
        assert_eq!(profiles.list().expect("a list"), []);
    }

    #[test]
    fn saves_the_settings_and_keeps_the_comments_of_the_file() {
        let directory = Directory::new();
        let profile = Profiles::new(Home::at(directory.path()))
            .start("main")
            .expect("a profile");
        let larger = Settings::default().with_larger_text();

        save(&profile, larger).expect("saved");

        let text = profile.read_settings().expect("a file").expect("a text");
        assert_eq!(text, TEMPLATE.replace("text-size = 100", "text-size = 110"));

        profile
            .write_settings("# mine\ntext-size = 110\n")
            .expect("written");
        save(&profile, Settings::default()).expect("saved");
        let text = profile.read_settings().expect("a file").expect("a text");
        assert!(text.starts_with("# mine\ntext-size = 100\n"), "{text}");
    }

    async fn next(changes: &mut (impl Stream<Item = String> + Unpin)) -> Option<String> {
        tokio::time::timeout(Duration::from_secs(20), changes.next())
            .await
            .expect("a change in time")
    }

    #[test]
    fn tells_when_the_text_of_a_file_changes() {
        let directory = Directory::new();
        let file = directory.join("settings.toml");
        std::fs::write(&file, "clock = 24\n").expect("a file");
        let runtime = tokio::runtime::Runtime::new().expect("a runtime");

        runtime.block_on(async {
            let mut changes = Box::pin(watch(file.clone(), Duration::from_millis(10)));

            std::fs::write(&file, "clock = 12\n").expect("a file");
            assert_eq!(next(&mut changes).await.as_deref(), Some("clock = 12\n"));

            std::fs::remove_file(&file).expect("removed");
            tokio::time::sleep(Duration::from_millis(50)).await;
            std::fs::write(&file, "clock = 12\n").expect("a file");
            tokio::time::sleep(Duration::from_millis(50)).await;
            std::fs::write(&file, "clock = 24\n").expect("a file");
            assert_eq!(next(&mut changes).await.as_deref(), Some("clock = 24\n"));
        });
    }

    #[test]
    fn a_different_event_is_not_a_shortcut() {
        let enter = pressed(Key::Named(Named::Enter), Modifiers::empty());
        let letter = pressed(Key::Character("a".into()), Modifiers::empty());
        let released = Event::Keyboard(keyboard::Event::KeyReleased {
            key: Key::Named(Named::Tab),
            modified_key: Key::Named(Named::Tab),
            physical_key: key::Physical::Unidentified(key::NativeCode::Unidentified),
            location: Location::Standard,
            modifiers: Modifiers::empty(),
        });
        let mouse = Event::Mouse(mouse::Event::CursorLeft);

        for event in [enter, letter, released, mouse] {
            assert!(shortcut(&event, Status::Ignored).is_none(), "{event:?}");
        }
    }

    #[test]
    fn the_application_obeys_the_mode_of_the_system() {
        let modes = [
            (theme::Mode::Dark, Mode::Dark),
            (theme::Mode::Light, Mode::Light),
            (theme::Mode::None, Mode::Light),
        ];
        for (system, expected) in modes {
            assert!(
                matches!(mode_changed(system), Message::ModeChanged(mode) if mode == expected),
                "{system:?}"
            );
        }
    }
}

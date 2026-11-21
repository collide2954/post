use std::fmt;
use std::fs::{self, File, TryLockError};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use post_crypto::Seed;
use post_proto::{Key, Name};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::home::Home;

const PROFILES: &str = "profiles";
const STATE: &str = "state.toml";
const PROFILE: &str = "profile.toml";
const SEED: &str = "seed";
const SERVERS: &str = "servers.toml";
const SETTINGS: &str = "settings.toml";
const LOCK: &str = "lock";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProfileId(u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileEntry {
    pub id: ProfileId,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Profiles {
    home: Home,
}

#[derive(Debug)]
pub struct Profile {
    entry: ProfileEntry,
    directory: PathBuf,
    settings: PathBuf,
    seed: Seed,
    servers: Vec<KnownServer>,
    _lock: File,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnownServer {
    pub address: String,
    pub key: Key,
    pub name: Name,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate: Option<String>,
}

#[derive(Debug)]
pub enum ProfileError {
    File(io::Error),
    InUse,
    NotFound,
    Damaged(&'static str),
    Name(NameProblem),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameProblem {
    Empty,
    TooLong,
    Character,
    Taken,
}

#[derive(Serialize, Deserialize)]
struct ProfileFile {
    name: String,
}

#[derive(Default, Serialize, Deserialize)]
struct ServersFile {
    #[serde(default, rename = "server")]
    servers: Vec<KnownServer>,
}

#[derive(Default, Serialize, Deserialize)]
struct StateFile {
    #[serde(default, rename = "last-profile")]
    last_profile: Option<u32>,
}

impl Profiles {
    pub const MAX_NAME_CHARACTERS: usize = 30;

    #[must_use]
    pub fn new(home: Home) -> Self {
        Self { home }
    }

    pub fn list(&self) -> Result<Vec<ProfileEntry>, ProfileError> {
        let directory = self.home.data.join(PROFILES);
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let mut profiles = Vec::new();
        for entry in entries {
            let name = entry?.file_name();
            let Some(id) = name.to_str().and_then(|name| name.parse().ok()) else {
                continue;
            };
            if let Ok(file) = read::<ProfileFile>(&directory.join(&name).join(PROFILE), "profile") {
                profiles.push(ProfileEntry {
                    id: ProfileId(id),
                    name: file.name,
                });
            }
        }
        profiles.sort_by_key(|profile| profile.id);
        Ok(profiles)
    }

    pub fn create(&self, name: &str) -> Result<ProfileId, ProfileError> {
        let profiles = self.list()?;
        let name = examine_name(name, &profiles, None)?;
        let id = ProfileId(profiles.last().map_or(1, |last| last.id.0 + 1));
        let directory = self.directory(id);
        make_private_directory(&directory)?;
        write(
            &directory.join(SEED),
            &format!("{}\n", *Seed::generate().encode()),
            true,
        )?;
        write_toml(&directory.join(PROFILE), &ProfileFile { name })?;
        Ok(id)
    }

    pub fn open(&self, id: ProfileId) -> Result<Profile, ProfileError> {
        let directory = self.directory(id);
        let file: ProfileFile = match read(&directory.join(PROFILE), "profile") {
            Err(ProfileError::File(error)) if error.kind() == io::ErrorKind::NotFound => {
                return Err(ProfileError::NotFound);
            }
            other => other?,
        };
        let lock = lock(&directory)?;
        let seed = fs::read_to_string(directory.join(SEED))?;
        let seed = Seed::decode(&seed).map_err(|_| ProfileError::Damaged("seed"))?;
        let servers = match read::<ServersFile>(&directory.join(SERVERS), "servers") {
            Err(ProfileError::File(error)) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            other => other?.servers,
        };
        write_toml(
            &self.home.data.join(STATE),
            &StateFile {
                last_profile: Some(id.0),
            },
        )?;
        Ok(Profile {
            entry: ProfileEntry {
                id,
                name: file.name,
            },
            settings: self
                .home
                .settings
                .join(PROFILES)
                .join(id.0.to_string())
                .join(SETTINGS),
            directory,
            seed,
            servers,
            _lock: lock,
        })
    }

    pub fn open_named(&self, name: &str) -> Result<Profile, ProfileError> {
        let wanted = name.trim().to_lowercase();
        let found = self
            .list()?
            .into_iter()
            .find(|profile| profile.name.to_lowercase() == wanted)
            .ok_or(ProfileError::NotFound)?;
        self.open(found.id)
    }

    pub fn start(&self, first_name: &str) -> Result<Profile, ProfileError> {
        let profiles = self.list()?;
        let Some(first) = profiles.first() else {
            return self.open(self.create(first_name)?);
        };
        let last = read::<StateFile>(&self.home.data.join(STATE), "state")
            .ok()
            .and_then(|state| state.last_profile)
            .map(ProfileId)
            .filter(|last| profiles.iter().any(|profile| profile.id == *last));
        self.open(last.unwrap_or(first.id))
    }

    pub fn rename(&self, id: ProfileId, name: &str) -> Result<(), ProfileError> {
        let profiles = self.list()?;
        if !profiles.iter().any(|profile| profile.id == id) {
            return Err(ProfileError::NotFound);
        }
        let name = examine_name(name, &profiles, Some(id))?;
        write_toml(&self.directory(id).join(PROFILE), &ProfileFile { name })
    }

    pub fn delete(&self, id: ProfileId) -> Result<(), ProfileError> {
        let directory = self.directory(id);
        if !directory.join(PROFILE).is_file() {
            return Err(ProfileError::NotFound);
        }
        drop(lock(&directory)?);
        fs::remove_dir_all(&directory)?;
        let settings = self.home.settings.join(PROFILES).join(id.0.to_string());
        if settings.is_dir() {
            fs::remove_dir_all(settings)?;
        }
        Ok(())
    }

    fn directory(&self, id: ProfileId) -> PathBuf {
        self.home.data.join(PROFILES).join(id.0.to_string())
    }
}

impl Profile {
    #[must_use]
    pub fn id(&self) -> ProfileId {
        self.entry.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.entry.name
    }

    #[must_use]
    pub fn seed(&self) -> &Seed {
        &self.seed
    }

    #[must_use]
    pub fn servers(&self) -> &[KnownServer] {
        &self.servers
    }

    #[must_use]
    pub fn settings_file(&self) -> &Path {
        &self.settings
    }

    pub fn remember(&mut self, server: KnownServer) -> Result<(), ProfileError> {
        self.servers
            .retain(|known| known.address != server.address && known.key != server.key);
        self.servers.insert(0, server);
        self.save()
    }

    pub fn forget(&mut self, address: &str) -> Result<(), ProfileError> {
        self.servers.retain(|known| known.address != address);
        self.save()
    }

    fn save(&self) -> Result<(), ProfileError> {
        let file = ServersFile {
            servers: self.servers.clone(),
        };
        write_toml(&self.directory.join(SERVERS), &file)
    }
}

fn examine_name(
    name: &str,
    profiles: &[ProfileEntry],
    except: Option<ProfileId>,
) -> Result<String, ProfileError> {
    let name = name.trim();
    let problem = if name.is_empty() {
        NameProblem::Empty
    } else if name.chars().count() > Profiles::MAX_NAME_CHARACTERS {
        NameProblem::TooLong
    } else if name.chars().any(char::is_control) {
        NameProblem::Character
    } else if profiles.iter().any(|profile| {
        Some(profile.id) != except && profile.name.to_lowercase() == name.to_lowercase()
    }) {
        NameProblem::Taken
    } else {
        return Ok(name.to_owned());
    };
    Err(ProfileError::Name(problem))
}

fn lock(directory: &Path) -> Result<File, ProfileError> {
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(directory.join(LOCK))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(ProfileError::InUse),
        Err(TryLockError::Error(error)) => Err(error.into()),
    }
}

fn make_private_directory(directory: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(directory)
}

fn read<T: DeserializeOwned>(path: &Path, what: &'static str) -> Result<T, ProfileError> {
    let text = fs::read_to_string(path)?;
    toml::from_str(&text).map_err(|_| ProfileError::Damaged(what))
}

fn write_toml<T: Serialize>(path: &Path, value: &T) -> Result<(), ProfileError> {
    let text = toml::to_string(value).map_err(io::Error::other)?;
    write(path, &text, false)
}

fn write(path: &Path, text: &str, private: bool) -> Result<(), ProfileError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let draft = path.with_extension("new");
    let mut options = File::options();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    if private {
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    }
    let _ = private;
    let mut file = options.open(&draft)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::rename(&draft, path)?;
    Ok(())
}

impl From<io::Error> for ProfileError {
    fn from(error: io::Error) -> Self {
        Self::File(error)
    }
}

impl fmt::Display for ProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File(error) => write!(formatter, "a file of the profile has a problem: {error}"),
            Self::InUse => formatter.write_str("the profile is open in a different window"),
            Self::NotFound => formatter.write_str("the profile does not exist"),
            Self::Damaged(what) => write!(formatter, "the {what} file of the profile is not valid"),
            Self::Name(NameProblem::Empty) => formatter.write_str("a profile must have a name"),
            Self::Name(NameProblem::TooLong) => write!(
                formatter,
                "a profile name can have a maximum of {} characters",
                Profiles::MAX_NAME_CHARACTERS
            ),
            Self::Name(NameProblem::Character) => {
                formatter.write_str("a profile name cannot contain a control character")
            }
            Self::Name(NameProblem::Taken) => {
                formatter.write_str("a different profile has this name")
            }
        }
    }
}

impl std::error::Error for ProfileError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use post_crypto::ServerKey;
    use post_proto::{Key, Name};
    use post_testing::Directory;

    use super::{KnownServer, NameProblem, ProfileError, ProfileId, Profiles};
    use crate::home::Home;

    fn profiles(directory: &Directory) -> Profiles {
        Profiles::new(Home::at(directory.path()))
    }

    fn server_key(number: u8) -> Key {
        ServerKey::from_bytes(&[number; 32]).public()
    }

    fn server(address: &str, number: u8, name: &str) -> KnownServer {
        KnownServer {
            address: address.to_owned(),
            key: server_key(number),
            name: Name::parse(name).expect(name),
            certificate: None,
        }
    }

    fn names(profiles: &Profiles) -> Vec<String> {
        profiles
            .list()
            .expect("a list")
            .into_iter()
            .map(|profile| profile.name)
            .collect()
    }

    #[test]
    fn makes_the_first_profile_at_the_first_start() {
        let directory = Directory::new();
        let profiles = profiles(&directory);
        assert_eq!(names(&profiles), Vec::<String>::new());

        let profile = profiles.start("main").expect("a profile");

        assert_eq!(profile.name(), "main");
        assert_eq!(profile.id(), ProfileId(1));
        assert_eq!(profile.servers(), []);
        assert_eq!(names(&profiles), ["main"]);
        let seed = fs::read_to_string(directory.join("profiles").join("1").join("seed"))
            .expect("a seed file");
        assert_eq!(seed, format!("{}\n", *profile.seed().encode()));
        assert_eq!(
            profile.settings_file(),
            directory.join("profiles").join("1").join("settings.toml")
        );
    }

    #[test]
    fn opens_the_same_profile_with_the_same_seed_at_the_next_start() {
        let directory = Directory::new();
        let profiles = profiles(&directory);
        let key = server_key(1);
        let first = profiles.start("main").expect("a profile");
        let account = first.seed().account_key(&key).public();
        drop(first);

        let second = profiles.start("a different name").expect("a profile");

        assert_eq!(second.name(), "main");
        assert_eq!(second.seed().account_key(&key).public(), account);
        assert_eq!(names(&profiles), ["main"]);
    }

    #[cfg(unix)]
    #[test]
    fn lets_only_the_user_read_the_seed() {
        use std::os::unix::fs::PermissionsExt;

        let directory = Directory::new();
        let _profile = profiles(&directory).start("main").expect("a profile");

        let profile = directory.join("profiles").join("1");
        let seed = fs::metadata(profile.join("seed")).expect("a seed file");
        assert_eq!(seed.permissions().mode() & 0o777, 0o600);
        let directory = fs::metadata(profile).expect("a directory");
        assert_eq!(directory.permissions().mode() & 0o777, 0o700);
    }

    #[test]
    fn gives_each_profile_its_own_seed_and_number() {
        let directory = Directory::new();
        let profiles = profiles(&directory);
        let key = server_key(1);

        let work = profiles.create("  work ").expect("a profile");
        let personal = profiles.create("Personal").expect("a profile");

        assert_eq!((work, personal), (ProfileId(1), ProfileId(2)));
        assert_eq!(names(&profiles), ["work", "Personal"]);
        let first = profiles.open(work).expect("open");
        let second = profiles.open(personal).expect("open");
        assert_ne!(
            first.seed().account_key(&key).public(),
            second.seed().account_key(&key).public()
        );
    }

    #[test]
    fn refuses_a_profile_name_that_breaks_a_rule() {
        let directory = Directory::new();
        let profiles = profiles(&directory);
        profiles.create("Work").expect("a profile");
        let long = "x".repeat(31);

        for (name, problem) in [
            ("", NameProblem::Empty),
            ("   ", NameProblem::Empty),
            (long.as_str(), NameProblem::TooLong),
            ("a\tb", NameProblem::Character),
            ("work", NameProblem::Taken),
            (" WORK ", NameProblem::Taken),
        ] {
            let result = profiles.create(name);
            assert!(
                matches!(result, Err(ProfileError::Name(found)) if found == problem),
                "{name:?}: {result:?}"
            );
        }
        assert!(profiles.create(&"\u{f1}".repeat(30)).is_ok());
        assert_eq!(profiles.list().expect("a list").len(), 2);
    }

    #[test]
    fn opens_a_profile_only_one_time() {
        let directory = Directory::new();
        let profiles = profiles(&directory);
        let id = profiles.create("main").expect("a profile");

        let open = profiles.open(id).expect("open");
        assert!(matches!(profiles.open(id), Err(ProfileError::InUse)));
        assert!(matches!(profiles.delete(id), Err(ProfileError::InUse)));

        drop(open);
        assert!(profiles.open(id).is_ok());
    }

    #[test]
    fn starts_with_the_profile_that_was_last_in_use() {
        let directory = Directory::new();
        let profiles = profiles(&directory);
        let work = profiles.create("work").expect("a profile");
        let personal = profiles.create("personal").expect("a profile");
        assert_eq!(profiles.start("x").expect("a profile").id(), work);

        drop(profiles.open(personal).expect("open"));
        assert_eq!(profiles.start("x").expect("a profile").id(), personal);

        profiles.delete(personal).expect("deleted");
        assert_eq!(profiles.start("x").expect("a profile").id(), work);
    }

    #[test]
    fn opens_a_profile_by_its_name() {
        let directory = Directory::new();
        let profiles = profiles(&directory);
        profiles.create("work").expect("a profile");
        let personal = profiles.create("Personal").expect("a profile");

        assert_eq!(
            profiles.open_named(" personal ").expect("open").id(),
            personal
        );
        assert!(matches!(
            profiles.open_named("garden"),
            Err(ProfileError::NotFound)
        ));
    }

    #[test]
    fn changes_the_name_of_a_profile() {
        let directory = Directory::new();
        let profiles = profiles(&directory);
        let work = profiles.create("work").expect("a profile");
        let personal = profiles.create("personal").expect("a profile");

        profiles.rename(work, "Office").expect("a new name");
        profiles
            .rename(work, "office")
            .expect("the same name in a different case");

        assert_eq!(names(&profiles), ["office", "personal"]);
        assert!(matches!(
            profiles.rename(personal, "OFFICE"),
            Err(ProfileError::Name(NameProblem::Taken))
        ));
        assert!(matches!(
            profiles.rename(ProfileId(9), "garden"),
            Err(ProfileError::NotFound)
        ));
    }

    #[test]
    fn deletes_a_profile_with_its_files() {
        let directory = Directory::new();
        let profiles = profiles(&directory);
        let work = profiles.create("work").expect("a profile");
        let personal = profiles.create("personal").expect("a profile");
        let settings = profiles
            .open(work)
            .expect("open")
            .settings_file()
            .to_owned();
        fs::write(&settings, "# settings\n").expect("a settings file");

        profiles.delete(work).expect("deleted");

        assert_eq!(names(&profiles), ["personal"]);
        assert!(!directory.join("profiles").join("1").exists());
        assert!(matches!(profiles.open(work), Err(ProfileError::NotFound)));
        assert!(matches!(profiles.delete(work), Err(ProfileError::NotFound)));
        assert_eq!(profiles.create("garden").expect("a profile"), ProfileId(3));
        assert_eq!(profiles.open(personal).expect("open").name(), "personal");
    }

    #[test]
    fn remembers_the_servers_with_the_newest_one_first() {
        let directory = Directory::new();
        let profiles = profiles(&directory);
        let mut profile = profiles.start("main").expect("a profile");

        profile
            .remember(server("chat.example.org", 1, "ana"))
            .expect("saved");
        profile
            .remember(server("localhost:7678", 2, "ana-dev"))
            .expect("saved");
        profile
            .remember(server("chat.example.org", 1, "anna"))
            .expect("saved");
        let expected = [
            server("chat.example.org", 1, "anna"),
            server("localhost:7678", 2, "ana-dev"),
        ];
        assert_eq!(profile.servers(), expected);

        drop(profile);
        let mut profile = profiles.start("main").expect("a profile");
        assert_eq!(profile.servers(), expected);

        profile.forget("chat.example.org").expect("saved");
        profile.forget("not-known.example.org").expect("saved");
        assert_eq!(profile.servers(), &expected[1..]);
    }

    #[test]
    fn keeps_one_entry_for_a_server_that_got_a_new_address() {
        let directory = Directory::new();
        let mut profile = profiles(&directory).start("main").expect("a profile");
        profile
            .remember(server("old.example.org", 1, "ana"))
            .expect("saved");

        profile
            .remember(server("new.example.org", 1, "ana"))
            .expect("saved");

        assert_eq!(profile.servers(), [server("new.example.org", 1, "ana")]);
    }

    #[test]
    fn keeps_the_fingerprint_of_a_private_certificate() {
        let directory = Directory::new();
        let profiles = profiles(&directory);
        let mut profile = profiles.start("main").expect("a profile");
        let with_certificate = KnownServer {
            certificate: Some("ab".repeat(32)),
            ..server("home.example.org:7678", 1, "ana")
        };

        profile.remember(with_certificate.clone()).expect("saved");
        drop(profile);

        let profile = profiles.start("main").expect("a profile");
        assert_eq!(profile.servers(), [with_certificate]);
    }

    #[test]
    fn reports_a_file_that_is_not_valid() {
        let directory = Directory::new();
        let profiles = profiles(&directory);
        let id = profiles.create("main").expect("a profile");
        let files = directory.join("profiles").join("1");

        fs::write(files.join("servers.toml"), "[[server]]\naddress = 7\n").expect("a file");
        assert!(matches!(
            profiles.open(id),
            Err(ProfileError::Damaged("servers"))
        ));

        fs::remove_file(files.join("servers.toml")).expect("removed");
        fs::write(files.join("seed"), "not a seed\n").expect("a file");
        assert!(matches!(
            profiles.open(id),
            Err(ProfileError::Damaged("seed"))
        ));

        fs::remove_file(files.join("seed")).expect("removed");
        assert!(matches!(profiles.open(id), Err(ProfileError::File(_))));

        fs::write(files.join("profile.toml"), "name = 7\n").expect("a file");
        assert_eq!(names(&profiles), Vec::<String>::new());
        assert!(matches!(
            profiles.open(id),
            Err(ProfileError::Damaged("profile"))
        ));
    }

    #[test]
    fn ignores_a_directory_that_is_not_a_profile() {
        let directory = Directory::new();
        let profiles = profiles(&directory);
        profiles.create("main").expect("a profile");
        fs::create_dir_all(directory.join("profiles").join("notes")).expect("a directory");
        fs::create_dir_all(directory.join("profiles").join("7")).expect("a directory");

        assert_eq!(names(&profiles), ["main"]);
        assert_eq!(profiles.create("work").expect("a profile"), ProfileId(2));
    }

    #[test]
    fn describes_each_error() {
        let texts = [
            (
                ProfileError::InUse,
                "the profile is open in a different window",
            ),
            (ProfileError::NotFound, "the profile does not exist"),
            (
                ProfileError::Damaged("seed"),
                "the seed file of the profile is not valid",
            ),
            (
                ProfileError::Name(NameProblem::Empty),
                "a profile must have a name",
            ),
            (
                ProfileError::Name(NameProblem::TooLong),
                "a profile name can have a maximum of 30 characters",
            ),
            (
                ProfileError::Name(NameProblem::Character),
                "a profile name cannot contain a control character",
            ),
            (
                ProfileError::Name(NameProblem::Taken),
                "a different profile has this name",
            ),
        ];
        for (error, text) in texts {
            assert_eq!(error.to_string(), text);
        }
        let file = ProfileError::from(std::io::Error::other("no"));
        assert_eq!(file.to_string(), "a file of the profile has a problem: no");
    }
}

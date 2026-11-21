use std::fmt;
use std::path::Path;
use std::time::Duration;

use post_crypto::ServerKey;
use post_proto::{Code, Key, Name, Role, Timestamp};
use rusqlite::types::Type;
use rusqlite::{Connection, OptionalExtension as _, Row, named_params, params};

const SCHEMA_VERSION: i64 = 1;
const SCHEMA: &str = "
CREATE TABLE server (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    secret BLOB NOT NULL
) STRICT;

CREATE TABLE invitation (
    code TEXT PRIMARY KEY,
    owner INTEGER NOT NULL,
    maker INTEGER REFERENCES account (id),
    created INTEGER NOT NULL,
    ends INTEGER,
    uses INTEGER,
    used INTEGER NOT NULL DEFAULT 0,
    revoked INTEGER
) STRICT;

CREATE TABLE account (
    id INTEGER PRIMARY KEY,
    key BLOB NOT NULL UNIQUE,
    name TEXT NOT NULL UNIQUE,
    owner INTEGER NOT NULL,
    created INTEGER NOT NULL,
    renamed INTEGER,
    invitation TEXT NOT NULL REFERENCES invitation (code)
) STRICT;

CREATE TABLE reserved_name (
    name TEXT PRIMARY KEY,
    account INTEGER NOT NULL REFERENCES account (id),
    until INTEGER NOT NULL
) STRICT;
";
const OPEN: &str =
    "revoked IS NULL AND (ends IS NULL OR ends > :now) AND (uses IS NULL OR used < uses)";
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const DAY: u64 = 24 * 60 * 60 * 1000;

pub type Stored<T> = Result<T, StoreError>;

pub struct Store {
    database: Connection,
    pub limits: Limits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub rename_interval_days: u64,
    pub name_reserved_days: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AccountId(i64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: AccountId,
    pub name: Name,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invitation {
    pub code: Code,
    pub by: Name,
    pub time: Timestamp,
    pub end: Option<Timestamp>,
    pub uses: Option<u32>,
    pub used: u32,
}

#[derive(Debug)]
pub enum StoreError {
    Database(rusqlite::Error),
    NewerData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterError {
    AccountExists,
    InvalidInvitation,
    NameTaken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameError {
    TooSoon,
    NameTaken,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            rename_interval_days: 30,
            name_reserved_days: 30,
        }
    }
}

impl Store {
    pub fn open(path: &Path) -> Stored<Self> {
        Self::prepare(Connection::open(path)?)
    }

    pub fn in_memory() -> Stored<Self> {
        Self::prepare(Connection::open_in_memory()?)
    }

    fn prepare(mut database: Connection) -> Stored<Self> {
        database.busy_timeout(BUSY_TIMEOUT)?;
        database.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        database.pragma_update(None, "foreign_keys", true)?;

        let version: i64 = database.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(StoreError::NewerData);
        }
        if version == 0 {
            let change = database.transaction()?;
            change.execute_batch(SCHEMA)?;
            change.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            change.commit()?;
        }
        Ok(Self {
            database,
            limits: Limits::default(),
        })
    }

    pub fn server_key(&mut self) -> Stored<ServerKey> {
        let change = self.database.transaction()?;
        let stored: Option<[u8; 32]> = change
            .query_row("SELECT secret FROM server WHERE id = 1", [], |row| {
                row.get(0)
            })
            .optional()?;
        let key = if let Some(secret) = stored {
            ServerKey::from_bytes(&secret)
        } else {
            let key = ServerKey::generate();
            change.execute(
                "INSERT INTO server (id, secret) VALUES (1, ?1)",
                [key.to_bytes().as_slice()],
            )?;
            key
        };
        change.commit()?;
        Ok(key)
    }

    pub fn account(&self, key: &Key) -> Stored<Option<Account>> {
        let account = self
            .database
            .query_row(
                "SELECT id, name, owner FROM account WHERE key = ?1",
                [key.as_bytes().as_slice()],
                account_from,
            )
            .optional()?;
        Ok(account)
    }

    pub fn register(
        &mut self,
        key: &Key,
        name: &Name,
        code: &Code,
        now: Timestamp,
    ) -> Stored<Result<Account, RegisterError>> {
        let change = self.database.transaction()?;
        let exists = change
            .prepare("SELECT 1 FROM account WHERE key = ?1")?
            .exists([key.as_bytes().as_slice()])?;
        if exists {
            return Ok(Err(RegisterError::AccountExists));
        }
        let owner: Option<bool> = change
            .query_row(
                &format!("SELECT owner FROM invitation WHERE code = :code AND {OPEN}"),
                named_params! { ":code": code.as_str(), ":now": millis(now) },
                |row| row.get(0),
            )
            .optional()?;
        let Some(owner) = owner else {
            return Ok(Err(RegisterError::InvalidInvitation));
        };
        let taken = change
            .prepare(
                "SELECT 1 FROM account WHERE name = ?1
                 UNION ALL SELECT 1 FROM reserved_name WHERE name = ?1 AND until > ?2",
            )?
            .exists(params![name.as_str(), millis(now)])?;
        if taken {
            return Ok(Err(RegisterError::NameTaken));
        }

        if owner {
            change.execute("UPDATE account SET owner = 0", [])?;
        }
        change.execute("DELETE FROM reserved_name WHERE name = ?1", [name.as_str()])?;
        change.execute(
            "INSERT INTO account (key, name, owner, created, invitation)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                key.as_bytes().as_slice(),
                name.as_str(),
                owner,
                millis(now),
                code.as_str()
            ],
        )?;
        let id = AccountId(change.last_insert_rowid());
        change.execute(
            "UPDATE invitation SET used = used + 1 WHERE code = ?1",
            [code.as_str()],
        )?;
        change.commit()?;

        Ok(Ok(Account {
            id,
            name: name.clone(),
            role: role(owner),
        }))
    }

    pub fn claim(&mut self, account: AccountId, code: &Code, now: Timestamp) -> Stored<bool> {
        let change = self.database.transaction()?;
        let used = change.execute(
            &format!(
                "UPDATE invitation SET used = used + 1
                 WHERE code = :code AND owner = 1 AND {OPEN}"
            ),
            named_params! { ":code": code.as_str(), ":now": millis(now) },
        )?;
        if used == 0 {
            return Ok(false);
        }
        change.execute("UPDATE account SET owner = (id = ?1)", [account.0])?;
        change.commit()?;
        Ok(true)
    }

    pub fn rename(
        &mut self,
        account: AccountId,
        name: &Name,
        now: Timestamp,
    ) -> Stored<Result<Name, RenameError>> {
        let interval = self.limits.rename_interval_days * DAY;
        let reserved_until = after(now, self.limits.name_reserved_days * DAY);
        let change = self.database.transaction()?;

        let (old, renamed): (Name, Option<Timestamp>) = change.query_row(
            "SELECT name, renamed FROM account WHERE id = ?1",
            [account.0],
            |row| Ok((name_from(row, 0)?, optional_time_from(row, 1)?)),
        )?;
        if renamed.is_some_and(|renamed| now < after(renamed, interval)) {
            return Ok(Err(RenameError::TooSoon));
        }
        let taken = change
            .prepare(
                "SELECT 1 FROM account WHERE name = ?1
                 UNION ALL
                 SELECT 1 FROM reserved_name WHERE name = ?1 AND until > ?2 AND account != ?3",
            )?
            .exists(params![name.as_str(), millis(now), account.0])?;
        if taken {
            return Ok(Err(RenameError::NameTaken));
        }

        change.execute("DELETE FROM reserved_name WHERE name = ?1", [name.as_str()])?;
        change.execute(
            "INSERT INTO reserved_name (name, account, until) VALUES (?1, ?2, ?3)",
            params![old.as_str(), account.0, millis(reserved_until)],
        )?;
        change.execute(
            "UPDATE account SET name = ?1, renamed = ?2 WHERE id = ?3",
            params![name.as_str(), millis(now), account.0],
        )?;
        change.commit()?;
        Ok(Ok(old))
    }

    pub fn invite(
        &mut self,
        by: AccountId,
        code: &Code,
        end: Option<Timestamp>,
        uses: Option<u32>,
        now: Timestamp,
    ) -> Stored<Invitation> {
        let change = self.database.transaction()?;
        let maker = change.query_row("SELECT name FROM account WHERE id = ?1", [by.0], |row| {
            name_from(row, 0)
        })?;
        change.execute(
            "INSERT INTO invitation (code, owner, maker, created, ends, uses)
             VALUES (?1, 0, ?2, ?3, ?4, ?5)",
            params![code.as_str(), by.0, millis(now), end.map(millis), uses],
        )?;
        change.commit()?;
        Ok(Invitation {
            code: code.clone(),
            by: maker,
            time: now,
            end,
            uses,
            used: 0,
        })
    }

    pub fn invitations(&self, now: Timestamp) -> Stored<Vec<Invitation>> {
        let mut query = self.database.prepare(&format!(
            "SELECT code, name, invitation.created, ends, uses, used
             FROM invitation JOIN account ON account.id = maker
             WHERE invitation.owner = 0 AND {OPEN}
             ORDER BY invitation.created, code"
        ))?;
        let invitations = query
            .query_map(named_params! { ":now": millis(now) }, invitation_from)?
            .collect::<Result<_, _>>()?;
        Ok(invitations)
    }

    pub fn revoke(&mut self, code: &Code, now: Timestamp) -> Stored<bool> {
        let changed = self.database.execute(
            &format!(
                "UPDATE invitation SET revoked = :now
                 WHERE code = :code AND owner = 0 AND {OPEN}"
            ),
            named_params! { ":code": code.as_str(), ":now": millis(now) },
        )?;
        Ok(changed == 1)
    }

    pub fn is_open(&self, code: &Code, now: Timestamp) -> Stored<bool> {
        let open = self
            .database
            .prepare(&format!(
                "SELECT 1 FROM invitation WHERE code = :code AND {OPEN}"
            ))?
            .exists(named_params! { ":code": code.as_str(), ":now": millis(now) })?;
        Ok(open)
    }

    pub fn owner_code(&mut self, fresh: &Code, now: Timestamp) -> Stored<Option<Code>> {
        let change = self.database.transaction()?;
        let has_owner = change
            .prepare("SELECT 1 FROM account WHERE owner = 1")?
            .exists([])?;
        if has_owner {
            return Ok(None);
        }
        let waiting = change
            .query_row(
                &format!("SELECT code FROM invitation WHERE owner = 1 AND {OPEN}"),
                named_params! { ":now": millis(now) },
                |row| code_from(row, 0),
            )
            .optional()?;
        let code = if let Some(code) = waiting {
            code
        } else {
            insert_owner_code(&change, fresh, now)?;
            fresh.clone()
        };
        change.commit()?;
        Ok(Some(code))
    }

    pub fn new_owner_code(&mut self, fresh: &Code, now: Timestamp) -> Stored<()> {
        let change = self.database.transaction()?;
        change.execute(
            "UPDATE invitation SET revoked = ?1 WHERE owner = 1 AND revoked IS NULL",
            [millis(now)],
        )?;
        insert_owner_code(&change, fresh, now)?;
        change.commit()?;
        Ok(())
    }
}

fn insert_owner_code(database: &Connection, code: &Code, now: Timestamp) -> rusqlite::Result<()> {
    database.execute(
        "INSERT INTO invitation (code, owner, created, uses) VALUES (?1, 1, ?2, 1)",
        params![code.as_str(), millis(now)],
    )?;
    Ok(())
}

fn role(owner: bool) -> Role {
    if owner { Role::Owner } else { Role::Member }
}

fn millis(time: Timestamp) -> i64 {
    i64::try_from(time.unix_millis()).expect("each timestamp is in the range of SQLite")
}

fn after(time: Timestamp, millis: u64) -> Timestamp {
    Timestamp::from_unix_millis(time.unix_millis().saturating_add(millis)).unwrap_or(Timestamp::MAX)
}

fn damaged(index: usize, kind: Type) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, kind, "the value is not valid".into())
}

fn name_from(row: &Row<'_>, index: usize) -> rusqlite::Result<Name> {
    let text: String = row.get(index)?;
    Name::parse(&text).map_err(|_| damaged(index, Type::Text))
}

fn code_from(row: &Row<'_>, index: usize) -> rusqlite::Result<Code> {
    let text: String = row.get(index)?;
    Code::parse(&text).map_err(|_| damaged(index, Type::Text))
}

fn optional_time_from(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<Timestamp>> {
    let Some(millis) = row.get::<_, Option<i64>>(index)? else {
        return Ok(None);
    };
    u64::try_from(millis)
        .ok()
        .and_then(Timestamp::from_unix_millis)
        .map(Some)
        .ok_or_else(|| damaged(index, Type::Integer))
}

fn time_from(row: &Row<'_>, index: usize) -> rusqlite::Result<Timestamp> {
    optional_time_from(row, index)?.ok_or_else(|| damaged(index, Type::Null))
}

fn account_from(row: &Row<'_>) -> rusqlite::Result<Account> {
    Ok(Account {
        id: AccountId(row.get(0)?),
        name: name_from(row, 1)?,
        role: role(row.get(2)?),
    })
}

fn invitation_from(row: &Row<'_>) -> rusqlite::Result<Invitation> {
    Ok(Invitation {
        code: code_from(row, 0)?,
        by: name_from(row, 1)?,
        time: time_from(row, 2)?,
        end: optional_time_from(row, 3)?,
        uses: row.get(4)?,
        used: row.get(5)?,
    })
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "the database has a problem: {error}"),
            Self::NewerData => {
                formatter.write_str("the data comes from a newer version of the server")
            }
        }
    }
}

impl std::error::Error for StoreError {}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    use post_proto::{Code, Key, Name, Role, Timestamp};

    use super::{Account, DAY, Invitation, Limits, RegisterError, RenameError, Store, StoreError};

    const START: u64 = 1_795_000_000_000;

    struct Directory(PathBuf);

    impl Directory {
        fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let name = format!(
                "post-store-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let path = std::env::temp_dir().join(name);
            std::fs::create_dir_all(&path).expect("a temporary directory");
            Self(path)
        }
    }

    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn day(number: u64) -> Timestamp {
        Timestamp::from_unix_millis(START + number * DAY).expect("a valid time")
    }

    fn before(time: Timestamp) -> Timestamp {
        Timestamp::from_unix_millis(time.unix_millis() - 1).expect("a valid time")
    }

    fn name(text: &str) -> Name {
        Name::parse(text).expect(text)
    }

    fn code(number: u32) -> Code {
        Code::parse(&format!("code{number:08}")).expect("a valid code")
    }

    fn key(byte: u8) -> Key {
        Key::from_bytes([byte; 32])
    }

    fn with_owner() -> (Store, Account) {
        let mut store = Store::in_memory().expect("a database in memory");
        let link = store
            .owner_code(&code(0), day(0))
            .expect("stored")
            .expect("the server has no owner");
        let owner = store
            .register(&key(1), &name("ana"), &link, day(0))
            .expect("stored")
            .expect("the owner link is open");
        (store, owner)
    }

    fn invite(store: &mut Store, by: &Account, number: u32, now: Timestamp) -> Invitation {
        store
            .invite(by.id, &code(number), None, None, now)
            .expect("stored")
    }

    fn join(store: &mut Store, owner: &Account, number: u8, who: &str, now: Timestamp) -> Account {
        let invitation = invite(store, owner, u32::from(number) + 1000, now);
        store
            .register(&key(number), &name(who), &invitation.code, now)
            .expect("stored")
            .expect("the invitation is open")
    }

    #[test]
    fn gives_the_user_of_the_owner_link_the_owner_role() {
        let (store, owner) = with_owner();

        assert_eq!(owner.name, name("ana"));
        assert_eq!(owner.role, Role::Owner);
        assert_eq!(store.account(&key(1)).expect("stored"), Some(owner));
    }

    #[test]
    fn does_not_know_a_key_without_an_account() {
        let (store, _owner) = with_owner();

        assert_eq!(store.account(&key(2)).expect("stored"), None);
    }

    #[test]
    fn gives_an_invited_user_the_member_role() {
        let (mut store, owner) = with_owner();

        let ben = join(&mut store, &owner, 2, "ben", day(1));

        assert_eq!(ben.role, Role::Member);
        assert_ne!(ben.id, owner.id);
        assert_eq!(store.account(&key(2)).expect("stored"), Some(ben));
        assert_eq!(
            store
                .account(&key(1))
                .expect("stored")
                .map(|owner| owner.role),
            Some(Role::Owner)
        );
    }

    #[test]
    fn refuses_a_second_account_for_one_key() {
        let (mut store, owner) = with_owner();
        let invitation = invite(&mut store, &owner, 1, day(0));

        let result = store
            .register(&key(1), &name("ana-2"), &invitation.code, day(0))
            .expect("stored");

        assert_eq!(result, Err(RegisterError::AccountExists));
    }

    #[test]
    fn refuses_an_invitation_that_is_not_open() {
        let (mut store, owner) = with_owner();
        let revoked = invite(&mut store, &owner, 1, day(0));
        assert!(store.revoke(&revoked.code, day(0)).expect("stored"));
        let ended = store
            .invite(owner.id, &code(2), Some(day(7)), None, day(0))
            .expect("stored");
        let used = store
            .invite(owner.id, &code(3), None, Some(1), day(0))
            .expect("stored");
        store
            .register(&key(2), &name("ben"), &used.code, day(0))
            .expect("stored")
            .expect("the first use is valid");

        for (invitation, now) in [
            (code(99), day(0)),
            (code(0), day(0)),
            (revoked.code, day(0)),
            (ended.code.clone(), day(7)),
            (ended.code, day(8)),
            (used.code, day(0)),
        ] {
            let result = store
                .register(&key(3), &name("cat"), &invitation, now)
                .expect("stored");
            assert_eq!(
                result,
                Err(RegisterError::InvalidInvitation),
                "{invitation}"
            );
        }
        assert_eq!(store.account(&key(3)).expect("stored"), None);
    }

    #[test]
    fn accepts_an_invitation_until_its_end_time() {
        let (mut store, owner) = with_owner();
        let invitation = store
            .invite(owner.id, &code(1), Some(day(7)), None, day(0))
            .expect("stored");

        let result = store
            .register(&key(2), &name("ben"), &invitation.code, before(day(7)))
            .expect("stored");

        assert_eq!(result.map(|account| account.name), Ok(name("ben")));
    }

    #[test]
    fn counts_each_use_of_an_invitation() {
        let (mut store, owner) = with_owner();
        let invitation = store
            .invite(owner.id, &code(1), None, Some(2), day(0))
            .expect("stored");
        assert_eq!(invitation.used, 0);

        for (number, who) in [(2, "ben"), (3, "cat")] {
            store
                .register(&key(number), &name(who), &invitation.code, day(0))
                .expect("stored")
                .expect("the invitation is open");
        }

        let third = store
            .register(&key(4), &name("dan"), &invitation.code, day(0))
            .expect("stored");
        assert_eq!(third, Err(RegisterError::InvalidInvitation));
        assert_eq!(store.invitations(day(0)).expect("stored"), []);
    }

    #[test]
    fn lets_many_users_use_an_invitation_without_a_limit() {
        let (mut store, owner) = with_owner();
        let invitation = invite(&mut store, &owner, 1, day(0));

        for number in 2..12 {
            let who = format!("user-{number}");
            store
                .register(&key(number), &name(&who), &invitation.code, day(0))
                .expect("stored")
                .expect("the invitation is open");
        }

        let listed = store.invitations(day(0)).expect("stored");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].used, 10);
    }

    #[test]
    fn refuses_a_name_that_a_different_account_has() {
        let (mut store, owner) = with_owner();
        let invitation = invite(&mut store, &owner, 1, day(0));

        let result = store
            .register(&key(2), &name("ana"), &invitation.code, day(0))
            .expect("stored");

        assert_eq!(result, Err(RegisterError::NameTaken));
        assert_eq!(store.invitations(day(0)).expect("stored")[0].used, 0);
    }

    #[test]
    fn examines_the_invitation_before_the_name() {
        let (mut store, _owner) = with_owner();

        let result = store
            .register(&key(2), &name("ana"), &code(99), day(0))
            .expect("stored");

        assert_eq!(result, Err(RegisterError::InvalidInvitation));
    }

    #[test]
    fn changes_a_name_and_gives_the_old_name() {
        let (mut store, owner) = with_owner();

        let old = store
            .rename(owner.id, &name("anna"), day(1))
            .expect("stored");

        assert_eq!(old, Ok(name("ana")));
        let account = store.account(&key(1)).expect("stored").expect("known");
        assert_eq!(account.name, name("anna"));
        assert_eq!(account.id, owner.id);
        assert_eq!(account.role, Role::Owner);
    }

    #[test]
    fn refuses_a_second_change_before_the_interval_ends() {
        let (mut store, owner) = with_owner();
        store
            .rename(owner.id, &name("anna"), day(1))
            .expect("stored")
            .expect("the first change is valid");

        let early = store
            .rename(owner.id, &name("ann"), before(day(31)))
            .expect("stored");
        assert_eq!(early, Err(RenameError::TooSoon));

        let later = store
            .rename(owner.id, &name("ann"), day(31))
            .expect("stored");
        assert_eq!(later, Ok(name("anna")));
    }

    #[test]
    fn keeps_the_old_name_for_its_user_for_some_time() {
        let (mut store, owner) = with_owner();
        let ben = join(&mut store, &owner, 2, "ben", day(0));
        store
            .rename(owner.id, &name("anna"), day(1))
            .expect("stored")
            .expect("valid");
        let invitation = invite(&mut store, &owner, 1, day(1));

        let renamed = store
            .rename(ben.id, &name("ana"), before(day(31)))
            .expect("stored");
        assert_eq!(renamed, Err(RenameError::NameTaken));
        let registered = store
            .register(&key(3), &name("ana"), &invitation.code, before(day(31)))
            .expect("stored");
        assert_eq!(registered, Err(RegisterError::NameTaken));

        let registered = store
            .register(&key(3), &name("ana"), &invitation.code, day(31))
            .expect("stored");
        assert_eq!(registered.map(|account| account.name), Ok(name("ana")));
    }

    #[test]
    fn gives_a_free_name_to_a_different_user_after_that_time() {
        let (mut store, owner) = with_owner();
        let ben = join(&mut store, &owner, 2, "ben", day(0));
        store
            .rename(owner.id, &name("anna"), day(1))
            .expect("stored")
            .expect("valid");

        let renamed = store.rename(ben.id, &name("ana"), day(31)).expect("stored");

        assert_eq!(renamed, Ok(name("ben")));
    }

    #[test]
    fn lets_a_user_take_back_the_reserved_name() {
        let (mut store, owner) = with_owner();
        store.limits = Limits {
            rename_interval_days: 1,
            name_reserved_days: 30,
        };
        store
            .rename(owner.id, &name("anna"), day(1))
            .expect("stored")
            .expect("valid");

        let back = store
            .rename(owner.id, &name("ana"), day(2))
            .expect("stored");
        assert_eq!(back, Ok(name("anna")));

        let again = store
            .rename(owner.id, &name("anna"), day(3))
            .expect("stored");
        assert_eq!(again, Ok(name("ana")));
    }

    #[test]
    fn refuses_a_change_to_a_name_in_use() {
        let (mut store, owner) = with_owner();
        let ben = join(&mut store, &owner, 2, "ben", day(0));

        for wanted in ["ana", "ben"] {
            let result = store.rename(ben.id, &name(wanted), day(1)).expect("stored");
            assert_eq!(result, Err(RenameError::NameTaken), "{wanted}");
        }
        assert_eq!(
            store.account(&key(2)).expect("stored").map(|ben| ben.name),
            Some(name("ben"))
        );
    }

    #[test]
    fn lists_the_open_invitations_in_time_sequence() {
        let (mut store, owner) = with_owner();
        let second = store
            .invite(owner.id, &code(2), Some(day(8)), Some(5), day(1))
            .expect("stored");
        let first = invite(&mut store, &owner, 9, day(0));
        let third = invite(&mut store, &owner, 1, day(1));
        assert_eq!(
            second,
            Invitation {
                code: code(2),
                by: name("ana"),
                time: day(1),
                end: Some(day(8)),
                uses: Some(5),
                used: 0,
            }
        );

        let listed = store.invitations(day(1)).expect("stored");

        assert_eq!(listed, [first, third, second]);
    }

    #[test]
    fn lists_no_invitation_that_is_not_open_and_no_owner_link() {
        let (mut store, owner) = with_owner();
        let open = invite(&mut store, &owner, 1, day(0));
        let revoked = invite(&mut store, &owner, 2, day(0));
        store.revoke(&revoked.code, day(0)).expect("stored");
        store
            .invite(owner.id, &code(3), Some(day(1)), None, day(0))
            .expect("stored");
        store.new_owner_code(&code(4), day(0)).expect("stored");

        assert_eq!(store.invitations(day(1)).expect("stored"), [open]);
    }

    #[test]
    fn shows_the_present_name_of_the_person_who_made_an_invitation() {
        let (mut store, owner) = with_owner();
        invite(&mut store, &owner, 1, day(0));
        store
            .rename(owner.id, &name("anna"), day(1))
            .expect("stored")
            .expect("valid");

        let listed = store.invitations(day(1)).expect("stored");

        assert_eq!(listed[0].by, name("anna"));
    }

    #[test]
    fn cancels_an_invitation_one_time() {
        let (mut store, owner) = with_owner();
        let invitation = invite(&mut store, &owner, 1, day(0));

        assert!(store.revoke(&invitation.code, day(0)).expect("stored"));
        assert!(!store.revoke(&invitation.code, day(0)).expect("stored"));
        assert!(!store.revoke(&code(99), day(0)).expect("stored"));
    }

    #[test]
    fn does_not_cancel_an_owner_link_as_an_invitation() {
        let (mut store, _owner) = with_owner();
        store.new_owner_code(&code(4), day(0)).expect("stored");

        assert!(!store.revoke(&code(4), day(0)).expect("stored"));
        assert!(store.is_open(&code(4), day(0)).expect("stored"));
    }

    #[test]
    fn tells_if_a_code_is_open() {
        let (mut store, owner) = with_owner();
        let invitation = store
            .invite(owner.id, &code(1), Some(day(7)), None, day(0))
            .expect("stored");

        assert!(store.is_open(&invitation.code, day(0)).expect("stored"));
        assert!(!store.is_open(&invitation.code, day(7)).expect("stored"));
        assert!(!store.is_open(&code(0), day(0)).expect("stored"));
        assert!(!store.is_open(&code(99), day(0)).expect("stored"));
    }

    #[test]
    fn gives_the_same_owner_link_until_a_person_uses_it() {
        let mut store = Store::in_memory().expect("a database in memory");

        let first = store.owner_code(&code(1), day(0)).expect("stored");
        let second = store.owner_code(&code(2), day(1)).expect("stored");

        assert_eq!(first, Some(code(1)));
        assert_eq!(second, Some(code(1)));
        assert!(!store.is_open(&code(2), day(1)).expect("stored"));
    }

    #[test]
    fn gives_no_owner_link_when_the_server_has_an_owner() {
        let (mut store, _owner) = with_owner();

        assert_eq!(store.owner_code(&code(1), day(0)).expect("stored"), None);
        assert!(!store.is_open(&code(1), day(0)).expect("stored"));
    }

    #[test]
    fn replaces_the_owner_link_on_demand() {
        let mut store = Store::in_memory().expect("a database in memory");
        store.owner_code(&code(1), day(0)).expect("stored");

        store.new_owner_code(&code(2), day(0)).expect("stored");

        assert!(!store.is_open(&code(1), day(0)).expect("stored"));
        assert_eq!(
            store.owner_code(&code(3), day(0)).expect("stored"),
            Some(code(2))
        );
    }

    #[test]
    fn makes_a_new_account_the_owner_with_a_new_owner_link() {
        let (mut store, _old) = with_owner();
        store.new_owner_code(&code(2), day(1)).expect("stored");

        let new = store
            .register(&key(2), &name("ben"), &code(2), day(1))
            .expect("stored")
            .expect("the owner link is open");

        assert_eq!(new.role, Role::Owner);
        let old = store.account(&key(1)).expect("stored").expect("known").role;
        assert_eq!(old, Role::Member);
    }

    #[test]
    fn makes_an_account_that_exists_the_owner_one_time() {
        let (mut store, owner) = with_owner();
        let ben = join(&mut store, &owner, 2, "ben", day(0));
        let cat = join(&mut store, &owner, 3, "cat", day(0));
        store.new_owner_code(&code(2), day(1)).expect("stored");

        assert!(store.claim(ben.id, &code(2), day(1)).expect("stored"));
        assert!(!store.claim(cat.id, &code(2), day(1)).expect("stored"));

        let roles: Vec<Role> = [1, 2, 3]
            .into_iter()
            .map(|number| {
                store
                    .account(&key(number))
                    .expect("stored")
                    .expect("known")
                    .role
            })
            .collect();
        assert_eq!(roles, [Role::Member, Role::Owner, Role::Member]);
    }

    #[test]
    fn gives_no_ownership_for_an_invitation() {
        let (mut store, owner) = with_owner();
        let ben = join(&mut store, &owner, 2, "ben", day(0));
        let invitation = invite(&mut store, &owner, 1, day(0));

        assert!(
            !store
                .claim(ben.id, &invitation.code, day(0))
                .expect("stored")
        );
        assert!(!store.claim(ben.id, &code(99), day(0)).expect("stored"));

        assert_eq!(store.invitations(day(0)).expect("stored")[0].used, 0);
        let role = store.account(&key(2)).expect("stored").expect("known").role;
        assert_eq!(role, Role::Member);
    }

    #[test]
    fn keeps_one_server_key() {
        let (mut store, _owner) = with_owner();

        let first = store.server_key().expect("stored").public();
        let second = store.server_key().expect("stored").public();

        assert_eq!(first, second);
    }

    #[test]
    fn keeps_its_data_in_a_file() {
        let directory = Directory::new();
        let path = directory.0.join("post.db");
        let (public, owner) = {
            let mut store = Store::open(&path).expect("a new database");
            let link = store
                .owner_code(&code(0), day(0))
                .expect("stored")
                .expect("the server has no owner");
            let owner = store
                .register(&key(1), &name("ana"), &link, day(0))
                .expect("stored")
                .expect("the owner link is open");
            (store.server_key().expect("stored").public(), owner)
        };

        let mut store = Store::open(&path).expect("the same database");

        assert_eq!(store.server_key().expect("stored").public(), public);
        assert_eq!(store.account(&key(1)).expect("stored"), Some(owner));
        assert_eq!(store.limits, Limits::default());
    }

    #[test]
    fn refuses_data_from_a_newer_version() {
        let directory = Directory::new();
        let path = directory.0.join("post.db");
        {
            let store = Store::open(&path).expect("a new database");
            store
                .database
                .pragma_update(None, "user_version", 99)
                .expect("a changed version");
        }

        let result = Store::open(&path);

        assert!(matches!(result, Err(StoreError::NewerData)));
    }

    #[test]
    fn reports_a_file_that_is_not_a_database() {
        let directory = Directory::new();
        let path = directory.0.join("post.db");
        std::fs::write(&path, "This file is not a database.").expect("a file");

        let result = Store::open(&path);

        assert!(matches!(result, Err(StoreError::Database(_))));
    }

    #[test]
    fn reports_a_value_that_is_not_valid() {
        let (mut store, owner) = with_owner();
        invite(&mut store, &owner, 1, day(0));

        store
            .database
            .execute("UPDATE invitation SET created = -1 WHERE owner = 0", [])
            .expect("a changed row");
        assert!(matches!(
            store.invitations(day(0)),
            Err(StoreError::Database(_))
        ));

        store
            .database
            .execute(
                "UPDATE invitation SET code = 'short', created = 0 WHERE owner = 0",
                [],
            )
            .expect("a changed row");
        assert!(matches!(
            store.invitations(day(0)),
            Err(StoreError::Database(_))
        ));

        store
            .database
            .execute("UPDATE account SET name = 'Not A Name'", [])
            .expect("a changed row");
        assert!(matches!(
            store.account(&key(1)),
            Err(StoreError::Database(_))
        ));
    }

    #[test]
    fn has_limits_of_thirty_days_by_default() {
        assert_eq!(
            Limits::default(),
            Limits {
                rename_interval_days: 30,
                name_reserved_days: 30,
            }
        );
    }

    #[test]
    fn describes_each_error() {
        assert_eq!(
            StoreError::NewerData.to_string(),
            "the data comes from a newer version of the server"
        );
        let error = StoreError::from(rusqlite::Error::QueryReturnedNoRows);
        assert_eq!(
            error.to_string(),
            "the database has a problem: Query returned no rows"
        );
    }
}

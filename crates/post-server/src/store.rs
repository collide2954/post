use std::fmt;
use std::fs::{File, TryLockError};
use std::io;
use std::path::Path;
use std::time::Duration;

use post_crypto::ServerKey;
use post_proto::{Code, Name, Timestamp};
use post_rules::{Account, AccountId, Change, Invitation, Limits, Reservation, Rules, Storage};
use rusqlite::types::Type;
use rusqlite::{Connection, OptionalExtension as _, Row, params};

mod channels;

pub use channels::ChannelRows;

const SCHEMA_VERSION: i64 = 2;
const STEPS: [(i64, &str); 2] = [(1, ACCOUNTS), (SCHEMA_VERSION, channels::SCHEMA)];
const ACCOUNTS: &str = "
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
const PUT_ACCOUNT: &str = "
INSERT INTO account (id, key, name, owner, created, renamed, invitation)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
ON CONFLICT (id) DO UPDATE SET
    key = excluded.key, name = excluded.name, owner = excluded.owner,
    created = excluded.created, renamed = excluded.renamed, invitation = excluded.invitation";
const PUT_INVITATION: &str = "
INSERT INTO invitation (code, owner, maker, created, ends, uses, used, revoked)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
ON CONFLICT (code) DO UPDATE SET
    owner = excluded.owner, maker = excluded.maker, created = excluded.created,
    ends = excluded.ends, uses = excluded.uses, used = excluded.used, revoked = excluded.revoked";
const PUT_RESERVATION: &str = "
INSERT INTO reserved_name (name, account, until) VALUES (?1, ?2, ?3)
ON CONFLICT (name) DO UPDATE SET account = excluded.account, until = excluded.until";
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub type Stored<T> = Result<T, StoreError>;

pub struct Store {
    database: Connection,
    _lock: Option<File>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Rows {
    pub accounts: Vec<Account>,
    pub invitations: Vec<Invitation>,
    pub reservations: Vec<Reservation>,
}

#[derive(Debug)]
pub enum StoreError {
    Database(rusqlite::Error),
    Lock(io::Error),
    InUse,
    NewerData,
    BrokenRules,
    CodeInUse,
}

impl Store {
    pub fn open(path: &Path) -> Stored<Self> {
        let lock = File::create(path.with_extension("lock")).map_err(StoreError::Lock)?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(StoreError::InUse),
            Err(TryLockError::Error(error)) => return Err(StoreError::Lock(error)),
        }
        Self::prepare(Connection::open(path)?, Some(lock))
    }

    pub fn in_memory() -> Stored<Self> {
        Self::prepare(Connection::open_in_memory()?, None)
    }

    fn prepare(mut database: Connection, lock: Option<File>) -> Stored<Self> {
        database.busy_timeout(BUSY_TIMEOUT)?;
        database.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        database.pragma_update(None, "foreign_keys", true)?;

        let version: i64 = database.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(StoreError::NewerData);
        }
        if version < SCHEMA_VERSION {
            let change = database.transaction()?;
            for (number, step) in STEPS {
                if version < number {
                    change.execute_batch(step)?;
                }
            }
            change.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            change.commit()?;
        }
        Ok(Self {
            database,
            _lock: lock,
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

    pub fn load(&self) -> Stored<Rows> {
        Ok(Rows {
            accounts: self.rows(
                "SELECT id, key, name, owner, created, renamed, invitation
                 FROM account ORDER BY id",
                account_from,
            )?,
            invitations: self.rows(
                "SELECT code, owner, maker, created, ends, uses, used, revoked
                 FROM invitation ORDER BY rowid",
                invitation_from,
            )?,
            reservations: self.rows(
                "SELECT name, account, until FROM reserved_name ORDER BY rowid",
                reservation_from,
            )?,
        })
    }

    pub fn rules(&self, limits: Limits) -> Stored<Rules> {
        let rows = self.load()?;
        Rules::load(
            limits,
            &rows.accounts,
            &rows.invitations,
            &rows.reservations,
        )
        .ok_or(StoreError::BrokenRules)
    }

    fn rows<T>(&self, query: &str, row: fn(&Row<'_>) -> rusqlite::Result<T>) -> Stored<Vec<T>> {
        let mut query = self.database.prepare(query)?;
        let rows = query.query_map([], row)?.collect::<Result<_, _>>()?;
        Ok(rows)
    }

    #[cfg(test)]
    pub(crate) fn lose_its_tables(&self) {
        self.database
            .execute_batch(
                "PRAGMA foreign_keys = OFF;
                 DROP TABLE next_channel; DROP TABLE reserved_channel_name;
                 DROP TABLE channel_invited; DROP TABLE channel_member; DROP TABLE channel;
                 DROP TABLE reserved_name; DROP TABLE account; DROP TABLE invitation;",
            )
            .expect("the tables go away");
    }
}

impl Storage for Store {
    type Error = StoreError;

    fn write(&mut self, changes: &[Change]) -> Stored<()> {
        let transaction = self.database.transaction()?;
        for change in changes {
            match change {
                Change::PutAccount(account) => transaction.execute(
                    PUT_ACCOUNT,
                    params![
                        number(account.id.0)?,
                        account.key.as_slice(),
                        account.name.as_str(),
                        account.owner,
                        millis(account.created),
                        account.renamed.map(millis),
                        account.invitation.as_str(),
                    ],
                )?,
                Change::PutInvitation(invitation) => transaction.execute(
                    PUT_INVITATION,
                    params![
                        invitation.code.as_str(),
                        invitation.owner,
                        invitation.maker.map(|maker| number(maker.0)).transpose()?,
                        millis(invitation.created),
                        invitation.end.map(millis),
                        invitation.uses,
                        invitation.used,
                        invitation.revoked.map(millis),
                    ],
                )?,
                Change::PutReservation(reservation) => transaction.execute(
                    PUT_RESERVATION,
                    params![
                        reservation.name.as_str(),
                        number(reservation.account.0)?,
                        millis(reservation.until),
                    ],
                )?,
                Change::DropReservation(name) => transaction
                    .execute("DELETE FROM reserved_name WHERE name = ?1", [name.as_str()])?,
            };
        }
        transaction.commit()?;
        Ok(())
    }
}

fn number(id: u64) -> rusqlite::Result<i64> {
    i64::try_from(id).map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

fn millis(time: Timestamp) -> i64 {
    i64::try_from(time.unix_millis()).expect("each timestamp is in the range of SQLite")
}

fn damaged(index: usize, kind: Type) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, kind, "the value is not valid".into())
}

fn number_from(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    u64::try_from(row.get::<_, i64>(index)?).map_err(|_| damaged(index, Type::Integer))
}

fn id_from(row: &Row<'_>, index: usize) -> rusqlite::Result<AccountId> {
    number_from(row, index).map(AccountId)
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
        id: id_from(row, 0)?,
        key: row.get(1)?,
        name: name_from(row, 2)?,
        owner: row.get(3)?,
        created: time_from(row, 4)?,
        renamed: optional_time_from(row, 5)?,
        invitation: code_from(row, 6)?,
    })
}

fn invitation_from(row: &Row<'_>) -> rusqlite::Result<Invitation> {
    let maker = match row.get::<_, Option<i64>>(2)? {
        Some(_) => Some(id_from(row, 2)?),
        None => None,
    };
    Ok(Invitation {
        code: code_from(row, 0)?,
        owner: row.get(1)?,
        maker,
        created: time_from(row, 3)?,
        end: optional_time_from(row, 4)?,
        uses: row.get(5)?,
        used: row.get(6)?,
        revoked: optional_time_from(row, 7)?,
    })
}

fn reservation_from(row: &Row<'_>) -> rusqlite::Result<Reservation> {
    Ok(Reservation {
        name: name_from(row, 0)?,
        account: id_from(row, 1)?,
        until: time_from(row, 2)?,
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
            Self::Lock(error) => write!(formatter, "the lock file has a problem: {error}"),
            Self::InUse => formatter
                .write_str("a different program uses the data of the server; stop it first"),
            Self::NewerData => {
                formatter.write_str("the data comes from a newer version of the server")
            }
            Self::BrokenRules => {
                formatter.write_str("the data of the server does not obey the rules of the server")
            }
            Self::CodeInUse => {
                formatter.write_str("a new code is in use already; do the operation again")
            }
        }
    }
}

impl std::error::Error for StoreError {}

#[cfg(test)]
mod tests {
    use post_proto::{Code, Name, Timestamp};
    use post_rules::{Account, AccountId, Change, Invitation, Reservation, Storage};
    use post_testing::Directory;

    use super::{Rows, Store, StoreError};

    const START: u64 = 1_795_000_000_000;

    fn time(number: u64) -> Timestamp {
        Timestamp::from_unix_millis(START + number).expect("a valid time")
    }

    fn name(text: &str) -> Name {
        Name::parse(text).expect(text)
    }

    fn code(number: u32) -> Code {
        Code::parse(&format!("code{number:08}")).expect("a valid code")
    }

    fn invitation(number: u32) -> Invitation {
        Invitation {
            code: code(number),
            owner: true,
            maker: None,
            created: time(0),
            end: None,
            uses: None,
            used: 0,
            revoked: None,
        }
    }

    fn account(id: u64, who: &str) -> Account {
        Account {
            id: AccountId(id),
            key: [u8::try_from(id).expect("a small number"); 32],
            name: name(who),
            owner: false,
            created: time(1),
            renamed: None,
            invitation: code(0),
        }
    }

    fn reservation(who: &str, account: u64) -> Reservation {
        Reservation {
            name: name(who),
            account: AccountId(account),
            until: time(9),
        }
    }

    fn with_rows() -> (Store, Rows) {
        let mut store = Store::in_memory().expect("a database in memory");
        let rows = Rows {
            accounts: vec![account(0, "ana"), account(5, "ben")],
            invitations: vec![
                invitation(0),
                Invitation {
                    owner: false,
                    maker: Some(AccountId(0)),
                    end: Some(time(7)),
                    uses: Some(3),
                    used: 2,
                    revoked: Some(time(4)),
                    ..invitation(1)
                },
            ],
            reservations: vec![reservation("ana-old", 0), reservation("ben-old", 5)],
        };
        store
            .write(&[
                Change::PutInvitation(rows.invitations[0].clone()),
                Change::PutAccount(rows.accounts[0].clone()),
                Change::PutAccount(rows.accounts[1].clone()),
                Change::PutInvitation(rows.invitations[1].clone()),
                Change::PutReservation(rows.reservations[0].clone()),
                Change::PutReservation(rows.reservations[1].clone()),
            ])
            .expect("stored");
        (store, rows)
    }

    #[test]
    fn has_no_rows_at_the_start() {
        let store = Store::in_memory().expect("a database in memory");

        assert_eq!(store.load().expect("loaded"), Rows::default());
    }

    #[test]
    fn gives_back_the_rows_that_it_stores() {
        let (store, rows) = with_rows();

        assert_eq!(store.load().expect("loaded"), rows);
    }

    #[test]
    fn replaces_a_row_that_has_the_same_identifier() {
        let (mut store, mut rows) = with_rows();
        rows.accounts[1] = Account {
            name: name("ben-2"),
            owner: true,
            renamed: Some(time(8)),
            ..account(5, "ben")
        };
        rows.invitations[0].used = 1;
        rows.reservations[0].until = time(20);

        store
            .write(&[
                Change::PutAccount(rows.accounts[1].clone()),
                Change::PutInvitation(rows.invitations[0].clone()),
                Change::PutReservation(rows.reservations[0].clone()),
            ])
            .expect("stored");

        assert_eq!(store.load().expect("loaded"), rows);
    }

    #[test]
    fn puts_a_reserved_name_at_the_end_when_it_comes_again() {
        let (mut store, mut rows) = with_rows();
        let first = rows.reservations.remove(0);

        store
            .write(&[
                Change::DropReservation(name("ana-old")),
                Change::DropReservation(name("not-there")),
                Change::PutReservation(first.clone()),
            ])
            .expect("stored");

        rows.reservations.push(first);
        assert_eq!(store.load().expect("loaded"), rows);
    }

    #[test]
    fn stores_all_changes_of_one_call_or_none() {
        let (mut store, rows) = with_rows();
        let orphan = Account {
            invitation: code(99),
            ..account(6, "cora")
        };

        let result = store.write(&[
            Change::PutAccount(account(7, "dan")),
            Change::PutAccount(orphan),
        ]);

        assert!(matches!(result, Err(StoreError::Database(_))));
        assert_eq!(store.load().expect("loaded"), rows);
    }

    #[test]
    fn refuses_an_identifier_that_is_too_large_for_the_database() {
        let (mut store, rows) = with_rows();

        let result = store.write(&[Change::PutAccount(account_with(u64::MAX - 1))]);

        assert!(matches!(result, Err(StoreError::Database(_))));
        assert_eq!(store.load().expect("loaded"), rows);
    }

    fn account_with(id: u64) -> Account {
        Account {
            id: AccountId(id),
            ..account(9, "eve")
        }
    }

    #[test]
    fn keeps_one_server_key() {
        let mut store = Store::in_memory().expect("a database in memory");

        let first = store.server_key().expect("stored").public();
        let second = store.server_key().expect("stored").public();

        assert_eq!(first, second);
    }

    #[test]
    fn keeps_its_data_in_a_file() {
        let directory = Directory::new();
        let path = directory.path().join("post.db");
        let (public, rows) = {
            let mut store = Store::open(&path).expect("a new database");
            let invitation = invitation(0);
            store
                .write(&[Change::PutInvitation(invitation.clone())])
                .expect("stored");
            let rows = Rows {
                invitations: vec![invitation],
                ..Rows::default()
            };
            (store.server_key().expect("stored").public(), rows)
        };

        let mut store = Store::open(&path).expect("the same database");

        assert_eq!(store.server_key().expect("stored").public(), public);
        assert_eq!(store.load().expect("loaded"), rows);
    }

    #[test]
    fn lets_only_one_program_use_a_file() {
        let directory = Directory::new();
        let path = directory.path().join("post.db");
        let first = Store::open(&path).expect("a new database");

        assert!(matches!(Store::open(&path), Err(StoreError::InUse)));

        drop(first);
        assert!(Store::open(&path).is_ok());
    }

    #[test]
    fn reports_a_lock_file_that_it_cannot_make() {
        let directory = Directory::new();
        let path = directory.path().join("no-such-directory").join("post.db");

        assert!(matches!(Store::open(&path), Err(StoreError::Lock(_))));
    }

    #[test]
    fn refuses_data_from_a_newer_version() {
        let directory = Directory::new();
        let path = directory.path().join("post.db");
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
        let path = directory.path().join("post.db");
        std::fs::write(&path, "This file is not a database.").expect("a file");

        let result = Store::open(&path);

        assert!(matches!(result, Err(StoreError::Database(_))));
    }

    #[test]
    fn reports_a_value_that_is_not_valid() {
        for damage in [
            "UPDATE invitation SET created = -1",
            "UPDATE invitation SET code = 'short' WHERE maker IS NOT NULL",
            "UPDATE invitation SET maker = -1 WHERE maker IS NOT NULL",
            "UPDATE account SET name = 'Not A Name' WHERE id = 5",
            "UPDATE account SET id = -1 WHERE id = 5",
            "UPDATE account SET created = -1 WHERE id = 5",
            "UPDATE reserved_name SET until = -1 WHERE account = 5",
            "UPDATE reserved_name SET name = 'Not A Name' WHERE account = 5",
        ] {
            let (store, _rows) = with_rows();
            store
                .database
                .execute_batch(&format!(
                    "PRAGMA foreign_keys = OFF; PRAGMA ignore_check_constraints = ON; {damage}"
                ))
                .unwrap_or_else(|error| panic!("{damage}: {error}"));

            let result = store.load();

            assert!(
                matches!(result, Err(StoreError::Database(_))),
                "{damage}: {result:?}"
            );
        }
    }

    #[test]
    fn describes_each_error() {
        let error = StoreError::from(rusqlite::Error::QueryReturnedNoRows);
        assert_eq!(
            error.to_string(),
            "the database has a problem: Query returned no rows"
        );
        let error = StoreError::Lock(std::io::Error::other("no access"));
        assert_eq!(error.to_string(), "the lock file has a problem: no access");
        assert_eq!(
            StoreError::InUse.to_string(),
            "a different program uses the data of the server; stop it first"
        );
        assert_eq!(
            StoreError::NewerData.to_string(),
            "the data comes from a newer version of the server"
        );
        assert_eq!(
            StoreError::BrokenRules.to_string(),
            "the data of the server does not obey the rules of the server"
        );
        assert_eq!(
            StoreError::CodeInUse.to_string(),
            "a new code is in use already; do the operation again"
        );
    }
}

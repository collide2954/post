use std::collections::BTreeMap;

use post_proto::{ChannelName, Timestamp, check_topic};
use post_rules::{
    AccountId, Channel, ChannelChange, ChannelId, ChannelLimits, ChannelStorage, Channels, Invited,
    Kind, Member, ReservedName,
};
use rusqlite::types::Type;
use rusqlite::{OptionalExtension as _, Row, params};

use super::{
    Store, StoreError, Stored, damaged, id_from, millis, number, number_from, optional_time_from,
    time_from,
};

pub(super) const SCHEMA: &str = "
CREATE TABLE channel (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    private INTEGER NOT NULL CHECK (private IN (0, 1)),
    owner INTEGER NOT NULL REFERENCES account (id),
    topic TEXT NOT NULL,
    created INTEGER NOT NULL,
    renamed INTEGER
) STRICT;

CREATE TABLE channel_member (
    channel INTEGER NOT NULL REFERENCES channel (id) ON DELETE CASCADE,
    account INTEGER NOT NULL REFERENCES account (id),
    joined INTEGER NOT NULL,
    PRIMARY KEY (channel, account)
) STRICT;

CREATE TABLE channel_invited (
    channel INTEGER NOT NULL REFERENCES channel (id) ON DELETE CASCADE,
    account INTEGER NOT NULL REFERENCES account (id),
    created INTEGER NOT NULL,
    PRIMARY KEY (channel, account)
) STRICT;

CREATE TABLE reserved_channel_name (
    name TEXT PRIMARY KEY,
    channel INTEGER NOT NULL,
    until INTEGER NOT NULL
) STRICT;

CREATE TABLE next_channel (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    number INTEGER NOT NULL
) STRICT;
";
const PUT_CHANNEL: &str = "
INSERT INTO channel (id, name, private, owner, topic, created, renamed)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
ON CONFLICT (id) DO UPDATE SET
    name = excluded.name, private = excluded.private, owner = excluded.owner,
    topic = excluded.topic, created = excluded.created, renamed = excluded.renamed";
const PUT_MEMBER: &str = "
INSERT INTO channel_member (channel, account, joined) VALUES (?1, ?2, ?3)
ON CONFLICT (channel, account) DO UPDATE SET joined = excluded.joined";
const PUT_INVITED: &str = "
INSERT INTO channel_invited (channel, account, created) VALUES (?1, ?2, ?3)
ON CONFLICT (channel, account) DO UPDATE SET created = excluded.created";
const PUT_RESERVED_NAME: &str = "
INSERT INTO reserved_channel_name (name, channel, until) VALUES (?1, ?2, ?3)
ON CONFLICT (name) DO UPDATE SET channel = excluded.channel, until = excluded.until";
const PUT_NEXT: &str = "
INSERT INTO next_channel (id, number) VALUES (1, ?1)
ON CONFLICT (id) DO UPDATE SET number = excluded.number";

#[derive(Debug, PartialEq, Eq)]
pub struct ChannelRows {
    pub next: ChannelId,
    pub channels: Vec<Channel>,
    pub reservations: Vec<ReservedName>,
}

impl Default for ChannelRows {
    fn default() -> Self {
        Self {
            next: ChannelId(0),
            channels: Vec::new(),
            reservations: Vec::new(),
        }
    }
}

type Person = (ChannelId, AccountId, Timestamp);

impl Store {
    pub fn load_channels(&self) -> Stored<ChannelRows> {
        let mut channels = self.rows(
            "SELECT id, name, private, owner, topic, created, renamed FROM channel ORDER BY id",
            channel_from,
        )?;
        let places: BTreeMap<ChannelId, usize> = channels
            .iter()
            .enumerate()
            .map(|(place, channel)| (channel.id, place))
            .collect();
        let place_of = |channel: ChannelId| {
            places
                .get(&channel)
                .copied()
                .ok_or_else(|| StoreError::Database(damaged(0, Type::Integer)))
        };
        for (channel, account, joined) in self.rows(
            "SELECT channel, account, joined FROM channel_member ORDER BY rowid",
            person_from,
        )? {
            channels[place_of(channel)?]
                .members
                .push(Member { account, joined });
        }
        for (channel, account, created) in self.rows(
            "SELECT channel, account, created FROM channel_invited ORDER BY rowid",
            person_from,
        )? {
            channels[place_of(channel)?]
                .invited
                .push(Invited { account, created });
        }
        let next = self
            .database
            .query_row("SELECT number FROM next_channel WHERE id = 1", [], |row| {
                number_from(row, 0)
            })
            .optional()?;
        Ok(ChannelRows {
            next: ChannelId(next.unwrap_or(0)),
            channels,
            reservations: self.rows(
                "SELECT name, channel, until FROM reserved_channel_name ORDER BY rowid",
                reserved_name_from,
            )?,
        })
    }

    pub fn channels(&self, limits: ChannelLimits) -> Stored<Channels> {
        let rows = self.load_channels()?;
        Channels::load(limits, rows.next, &rows.channels, &rows.reservations)
            .ok_or(StoreError::BrokenRules)
    }
}

impl ChannelStorage for Store {
    type Error = StoreError;

    fn write(&mut self, changes: &[ChannelChange]) -> Stored<()> {
        let transaction = self.database.transaction()?;
        for change in changes {
            match change {
                ChannelChange::PutNext(next) => transaction.execute(PUT_NEXT, [number(next.0)?])?,
                ChannelChange::PutChannel(head) => transaction.execute(
                    PUT_CHANNEL,
                    params![
                        number(head.id.0)?,
                        head.name.as_str(),
                        head.kind.is_private(),
                        number(head.owner.0)?,
                        head.topic,
                        millis(head.created),
                        head.renamed.map(millis),
                    ],
                )?,
                ChannelChange::DropChannel(id) => {
                    transaction.execute("DELETE FROM channel WHERE id = ?1", [number(id.0)?])?
                }
                ChannelChange::PutMember(id, member) => transaction.execute(
                    PUT_MEMBER,
                    params![
                        number(id.0)?,
                        number(member.account.0)?,
                        millis(member.joined)
                    ],
                )?,
                ChannelChange::DropMember(id, account) => transaction.execute(
                    "DELETE FROM channel_member WHERE channel = ?1 AND account = ?2",
                    [number(id.0)?, number(account.0)?],
                )?,
                ChannelChange::PutInvited(id, invited) => transaction.execute(
                    PUT_INVITED,
                    params![
                        number(id.0)?,
                        number(invited.account.0)?,
                        millis(invited.created)
                    ],
                )?,
                ChannelChange::DropInvited(id, account) => transaction.execute(
                    "DELETE FROM channel_invited WHERE channel = ?1 AND account = ?2",
                    [number(id.0)?, number(account.0)?],
                )?,
                ChannelChange::PutReservedName(reserved) => transaction.execute(
                    PUT_RESERVED_NAME,
                    params![
                        reserved.name.as_str(),
                        number(reserved.channel.0)?,
                        millis(reserved.until),
                    ],
                )?,
                ChannelChange::DropReservedName(name) => transaction.execute(
                    "DELETE FROM reserved_channel_name WHERE name = ?1",
                    [name.as_str()],
                )?,
            };
        }
        transaction.commit()?;
        Ok(())
    }
}

fn channel_name_from(row: &Row<'_>, index: usize) -> rusqlite::Result<ChannelName> {
    let text: String = row.get(index)?;
    ChannelName::parse(&text).map_err(|_| damaged(index, Type::Text))
}

fn channel_from(row: &Row<'_>) -> rusqlite::Result<Channel> {
    let topic: String = row.get(4)?;
    check_topic(&topic).map_err(|_| damaged(4, Type::Text))?;
    Ok(Channel {
        id: ChannelId(number_from(row, 0)?),
        name: channel_name_from(row, 1)?,
        kind: match row.get(2)? {
            0_i64 => Kind::Public,
            1 => Kind::Private,
            _ => return Err(damaged(2, Type::Integer)),
        },
        owner: id_from(row, 3)?,
        topic,
        created: time_from(row, 5)?,
        renamed: optional_time_from(row, 6)?,
        members: Vec::new(),
        invited: Vec::new(),
    })
}

fn person_from(row: &Row<'_>) -> rusqlite::Result<Person> {
    Ok((
        ChannelId(number_from(row, 0)?),
        id_from(row, 1)?,
        time_from(row, 2)?,
    ))
}

fn reserved_name_from(row: &Row<'_>) -> rusqlite::Result<ReservedName> {
    Ok(ReservedName {
        name: channel_name_from(row, 0)?,
        channel: ChannelId(number_from(row, 1)?),
        until: time_from(row, 2)?,
    })
}

#[cfg(test)]
mod tests {
    use post_proto::{ChannelName, Code, Name, Timestamp};
    use post_rules::{
        Account, AccountId, Change, Channel, ChannelChange, ChannelId, ChannelLimits,
        ChannelStorage as _, Invitation, Invited, Kind, Member, ReservedName,
    };
    use post_testing::Directory;
    use rusqlite::Connection;

    use super::ChannelRows;
    use crate::store::{Store, StoreError};

    const START: u64 = 1_795_000_000_000;
    const LIMITS: ChannelLimits = ChannelLimits {
        owned: 10,
        rename_interval: 1000,
        name_reserved: 1000,
    };
    const ANA: AccountId = AccountId(0);
    const BEN: AccountId = AccountId(5);
    const CY: AccountId = AccountId(7);

    fn time(number: u64) -> Timestamp {
        Timestamp::from_unix_millis(START + number).expect("a valid time")
    }

    fn name(text: &str) -> ChannelName {
        ChannelName::parse(text).expect(text)
    }

    fn code() -> Code {
        Code::parse("code00000000").expect("a valid code")
    }

    fn account(id: AccountId, who: &str) -> Change {
        Change::PutAccount(Account {
            id,
            key: [u8::try_from(id.0).expect("a small number"); 32],
            name: Name::parse(who).expect(who),
            owner: false,
            created: time(1),
            renamed: None,
            invitation: code(),
        })
    }

    fn with_accounts() -> Store {
        let mut store = Store::in_memory().expect("a database in memory");
        let invitation = Invitation {
            code: code(),
            owner: true,
            maker: None,
            created: time(0),
            end: None,
            uses: None,
            used: 0,
            revoked: None,
        };
        post_rules::Storage::write(
            &mut store,
            &[
                Change::PutInvitation(invitation),
                account(ANA, "ana"),
                account(BEN, "ben"),
                account(CY, "cy"),
            ],
        )
        .expect("stored");
        store
    }

    fn member(account: AccountId, joined: u64) -> Member {
        Member {
            account,
            joined: time(joined),
        }
    }

    fn channel(id: u64, title: &str, owner: AccountId) -> Channel {
        Channel {
            id: ChannelId(id),
            name: name(title),
            kind: Kind::Public,
            owner,
            topic: String::new(),
            created: time(2),
            renamed: None,
            members: vec![member(owner, 2)],
            invited: Vec::new(),
        }
    }

    fn reserved(title: &str, channel: u64) -> ReservedName {
        ReservedName {
            name: name(title),
            channel: ChannelId(channel),
            until: time(90),
        }
    }

    fn changes_of(channel: &Channel) -> Vec<ChannelChange> {
        let mut changes = vec![ChannelChange::PutChannel(channel.head())];
        changes.extend(
            channel
                .members
                .iter()
                .map(|member| ChannelChange::PutMember(channel.id, *member)),
        );
        changes.extend(
            channel
                .invited
                .iter()
                .map(|invited| ChannelChange::PutInvited(channel.id, *invited)),
        );
        changes
    }

    fn with_channels() -> (Store, ChannelRows) {
        let mut store = with_accounts();
        let rows = ChannelRows {
            next: ChannelId(10),
            channels: vec![
                channel(0, "general", ANA),
                Channel {
                    kind: Kind::Private,
                    topic: "Plans for the week".to_owned(),
                    renamed: Some(time(6)),
                    members: vec![member(BEN, 3), member(ANA, 4)],
                    invited: vec![Invited {
                        account: CY,
                        created: time(5),
                    }],
                    ..channel(3, "dev", BEN)
                },
            ],
            reservations: vec![reserved("dev-old", 3), reserved("gone", 9)],
        };
        let mut changes = vec![ChannelChange::PutNext(rows.next)];
        changes.extend(rows.channels.iter().flat_map(changes_of));
        changes.extend(
            rows.reservations
                .iter()
                .cloned()
                .map(ChannelChange::PutReservedName),
        );
        store.write(&changes).expect("stored");
        (store, rows)
    }

    #[test]
    fn has_no_channel_rows_at_the_start() {
        let store = Store::in_memory().expect("a database in memory");

        assert_eq!(
            store.load_channels().expect("loaded"),
            ChannelRows::default()
        );
        assert_eq!(
            store.channels(LIMITS).expect("loaded").rows(),
            (&[][..], &[][..])
        );
    }

    #[test]
    fn gives_back_the_channel_rows_that_it_stores() {
        let (store, rows) = with_channels();

        assert_eq!(store.load_channels().expect("loaded"), rows);
        assert_eq!(
            store.channels(LIMITS).expect("loaded").rows(),
            (&rows.channels[..], &rows.reservations[..])
        );
        assert_eq!(
            store.channels(LIMITS).expect("loaded").next(),
            ChannelId(10)
        );
    }

    #[test]
    fn replaces_the_head_of_a_channel_and_keeps_its_people() {
        let (mut store, mut rows) = with_channels();
        let dev = &mut rows.channels[1];
        dev.name = name("develop");
        dev.owner = ANA;
        dev.topic = "New plans".to_owned();
        dev.renamed = Some(time(8));
        rows.reservations[0].until = time(99);
        rows.next = ChannelId(11);

        store
            .write(&[
                ChannelChange::PutNext(ChannelId(11)),
                ChannelChange::PutChannel(rows.channels[1].head()),
                ChannelChange::PutReservedName(rows.reservations[0].clone()),
            ])
            .expect("stored");

        assert_eq!(store.load_channels().expect("loaded"), rows);
    }

    #[test]
    fn drops_a_member_an_invitation_and_a_reserved_name() {
        let (mut store, mut rows) = with_channels();

        store
            .write(&[
                ChannelChange::DropMember(ChannelId(3), ANA),
                ChannelChange::DropMember(ChannelId(3), CY),
                ChannelChange::DropInvited(ChannelId(3), CY),
                ChannelChange::DropInvited(ChannelId(0), CY),
                ChannelChange::DropReservedName(name("dev-old")),
                ChannelChange::DropReservedName(name("not-there")),
            ])
            .expect("stored");

        rows.channels[1].members.remove(1);
        rows.channels[1].invited.clear();
        rows.reservations.remove(0);
        assert_eq!(store.load_channels().expect("loaded"), rows);
    }

    #[test]
    fn puts_a_person_at_the_end_when_the_person_comes_again() {
        let (mut store, mut rows) = with_channels();
        let invited = Invited {
            account: ANA,
            created: time(7),
        };

        store
            .write(&[
                ChannelChange::DropMember(ChannelId(3), BEN),
                ChannelChange::PutMember(ChannelId(3), member(BEN, 9)),
                ChannelChange::PutInvited(ChannelId(3), invited),
                ChannelChange::DropInvited(ChannelId(3), CY),
                ChannelChange::PutInvited(ChannelId(3), rows.channels[1].invited[0]),
            ])
            .expect("stored");

        rows.channels[1].members = vec![member(ANA, 4), member(BEN, 9)];
        rows.channels[1].invited.insert(0, invited);
        assert_eq!(store.load_channels().expect("loaded"), rows);
    }

    #[test]
    fn drops_a_channel_with_its_people_and_keeps_its_reserved_names() {
        let (mut store, mut rows) = with_channels();

        store
            .write(&[
                ChannelChange::DropChannel(ChannelId(3)),
                ChannelChange::DropChannel(ChannelId(44)),
            ])
            .expect("stored");

        rows.channels.remove(1);
        assert_eq!(store.load_channels().expect("loaded"), rows);
        let people: i64 = store
            .database
            .query_row(
                "SELECT (SELECT count(*) FROM channel_member WHERE channel = 3)
                      + (SELECT count(*) FROM channel_invited WHERE channel = 3)",
                [],
                |row| row.get(0),
            )
            .expect("counted");
        assert_eq!(people, 0);
    }

    #[test]
    fn stores_all_channel_changes_of_one_call_or_none() {
        let (mut store, rows) = with_channels();

        for last in [
            ChannelChange::PutMember(ChannelId(44), member(ANA, 9)),
            ChannelChange::PutMember(ChannelId(0), member(AccountId(99), 9)),
            ChannelChange::PutChannel(channel(6, "general", ANA).head()),
            ChannelChange::PutChannel(channel(7, "new", AccountId(99)).head()),
            ChannelChange::PutChannel(channel(u64::MAX - 1, "large", ANA).head()),
        ] {
            let result = store.write(&[
                ChannelChange::PutChannel(channel(5, "talk", ANA).head()),
                last,
            ]);

            assert!(matches!(result, Err(StoreError::Database(_))), "{result:?}");
            assert_eq!(store.load_channels().expect("loaded"), rows);
        }
    }

    #[test]
    fn does_not_load_channel_rows_that_break_a_rule() {
        let mut store = with_accounts();
        store
            .write(&[
                ChannelChange::PutNext(ChannelId(1)),
                ChannelChange::PutChannel(channel(0, "general", ANA).head()),
            ])
            .expect("stored");

        assert_eq!(store.load_channels().expect("loaded").channels.len(), 1);
        assert!(matches!(
            store.channels(LIMITS),
            Err(StoreError::BrokenRules)
        ));

        store
            .write(&[
                ChannelChange::PutMember(ChannelId(0), member(ANA, 2)),
                ChannelChange::PutNext(ChannelId(0)),
            ])
            .expect("stored");
        assert!(matches!(
            store.channels(LIMITS),
            Err(StoreError::BrokenRules)
        ));
        store
            .write(&[ChannelChange::PutNext(ChannelId(1))])
            .expect("stored");
        assert!(store.channels(LIMITS).is_ok());
    }

    #[test]
    fn reports_a_channel_value_that_is_not_valid() {
        for damage in [
            "UPDATE channel SET name = 'Not A Name' WHERE id = 3",
            "UPDATE channel SET id = -1 WHERE id = 3",
            "UPDATE channel SET private = 2 WHERE id = 3",
            "UPDATE channel SET owner = -1 WHERE id = 3",
            "UPDATE channel SET topic = 'two' || char(10) || 'lines' WHERE id = 3",
            "UPDATE channel SET created = -1 WHERE id = 3",
            "UPDATE channel SET renamed = -1 WHERE id = 3",
            "UPDATE channel_member SET channel = 44 WHERE account = 5",
            "UPDATE channel_member SET account = -1 WHERE account = 5",
            "UPDATE channel_member SET joined = -1 WHERE account = 5",
            "UPDATE channel_invited SET channel = 44",
            "UPDATE channel_invited SET account = -1",
            "UPDATE channel_invited SET created = -1",
            "UPDATE reserved_channel_name SET name = 'x' WHERE channel = 9",
            "UPDATE reserved_channel_name SET channel = -1 WHERE channel = 9",
            "UPDATE reserved_channel_name SET until = -1 WHERE channel = 9",
            "UPDATE next_channel SET number = -1",
        ] {
            let (store, _rows) = with_channels();
            store
                .database
                .execute_batch(&format!(
                    "PRAGMA foreign_keys = OFF; PRAGMA ignore_check_constraints = ON; {damage}"
                ))
                .unwrap_or_else(|error| panic!("{damage}: {error}"));

            let result = store.load_channels();

            assert!(
                matches!(result, Err(StoreError::Database(_))),
                "{damage}: {result:?}"
            );
        }
    }

    #[test]
    fn adds_the_channel_tables_to_the_data_of_an_earlier_version() {
        let directory = Directory::new();
        let path = directory.path().join("post.db");
        {
            let database = Connection::open(&path).expect("a new database");
            database
                .execute_batch(crate::store::ACCOUNTS)
                .expect("the tables of the first version");
            database
                .pragma_update(None, "user_version", 1)
                .expect("the first version");
            database
                .execute(
                    "INSERT INTO invitation (code, owner, created) VALUES ('code00000000', 1, 5)",
                    [],
                )
                .expect("a row of the first version");
        }

        let mut store = Store::open(&path).expect("the same database");

        assert_eq!(store.load().expect("loaded").invitations.len(), 1);
        assert_eq!(
            store.load_channels().expect("loaded"),
            ChannelRows::default()
        );
        post_rules::Storage::write(&mut store, &[account(ANA, "ana")]).expect("stored");
        let mut changes = changes_of(&channel(0, "general", ANA));
        changes.push(ChannelChange::PutNext(ChannelId(1)));
        store.write(&changes).expect("stored");
        drop(store);
        let store = Store::open(&path).expect("the same database");
        let rows = store.load_channels().expect("loaded");
        assert_eq!((rows.channels.len(), rows.next), (1, ChannelId(1)));
    }
}

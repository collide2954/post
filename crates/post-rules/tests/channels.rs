use post_proto::{ChannelName, Timestamp};
use post_rules::{
    AccountId, Channel, ChannelChange, ChannelHead, ChannelId, ChannelInviteError, ChannelLimits,
    ChannelRenameError, ChannelStorage, Channels, CreateError, GiveError, InvitationError, Invited,
    JoinError, Kind, LeaveError, Member, OwnerError, RefuseError, ReservedName,
};

const START: u64 = 1_795_000_000_000;
const DAY: u64 = 24 * 60 * 60 * 1000;
const LIMITS: ChannelLimits = ChannelLimits {
    owned: 3,
    rename_interval: 30 * DAY,
    name_reserved: 30 * DAY,
};
const ANA: AccountId = AccountId(1);
const BEN: AccountId = AccountId(2);
const CY: AccountId = AccountId(3);

#[derive(Debug, PartialEq, Eq)]
struct Broken;

#[derive(Default)]
struct Memory {
    next: u64,
    channels: Vec<Channel>,
    reservations: Vec<ReservedName>,
    is_broken: bool,
}

impl Memory {
    fn channel(&mut self, id: ChannelId) -> &mut Channel {
        self.channels
            .iter_mut()
            .find(|channel| channel.id == id)
            .expect("the storage has the channel")
    }
}

impl ChannelStorage for Memory {
    type Error = Broken;

    fn write(&mut self, changes: &[ChannelChange]) -> Result<(), Broken> {
        if self.is_broken {
            return Err(Broken);
        }
        for change in changes {
            match change {
                ChannelChange::PutNext(next) => self.next = next.0,
                ChannelChange::PutChannel(head) => {
                    match self.channels.iter_mut().find(|row| row.id == head.id) {
                        Some(row) => {
                            row.name = head.name.clone();
                            row.owner = head.owner;
                            row.topic.clone_from(&head.topic);
                            row.renamed = head.renamed;
                        }
                        None => self.channels.push(Channel {
                            id: head.id,
                            name: head.name.clone(),
                            kind: head.kind,
                            owner: head.owner,
                            topic: head.topic.clone(),
                            created: head.created,
                            renamed: head.renamed,
                            members: Vec::new(),
                            invited: Vec::new(),
                        }),
                    }
                }
                ChannelChange::DropChannel(id) => self.channels.retain(|row| row.id != *id),
                ChannelChange::PutMember(id, member) => self.channel(*id).members.push(*member),
                ChannelChange::DropMember(id, account) => self
                    .channel(*id)
                    .members
                    .retain(|member| member.account != *account),
                ChannelChange::PutInvited(id, invited) => {
                    self.channel(*id).invited.push(*invited);
                }
                ChannelChange::DropInvited(id, account) => self
                    .channel(*id)
                    .invited
                    .retain(|invited| invited.account != *account),
                ChannelChange::PutReservedName(reserved) => {
                    self.reservations.push(reserved.clone());
                }
                ChannelChange::DropReservedName(name) => {
                    self.reservations.retain(|row| row.name != *name);
                }
            }
        }
        Ok(())
    }
}

struct Server {
    channels: Channels,
    memory: Memory,
}

fn name(text: &str) -> ChannelName {
    ChannelName::parse(text).expect(text)
}

fn day(number: u64) -> Timestamp {
    Timestamp::from_unix_millis(START + number * DAY).expect("a valid time")
}

impl Server {
    fn new() -> Self {
        Self {
            channels: Channels::new(LIMITS),
            memory: Memory::default(),
        }
    }

    fn with_channel(kind: Kind) -> (Self, ChannelId) {
        let mut server = Self::new();
        let id = server
            .create(ANA, "general", kind, day(0))
            .expect("the name is free");
        (server, id)
    }

    fn examine(&self) {
        let (channels, reservations) = self.channels.rows();
        assert_eq!(channels, self.memory.channels.as_slice());
        assert_eq!(reservations, self.memory.reservations.as_slice());
        let next = ChannelId(self.memory.next);
        assert_eq!(self.channels.next(), next);
        let loaded =
            Channels::load(LIMITS, next, channels, reservations).expect("the rows obey the rules");
        assert_eq!(loaded.rows(), self.channels.rows());
        assert_eq!(loaded.next(), next);
    }

    fn create(
        &mut self,
        by: AccountId,
        channel: &str,
        kind: Kind,
        now: Timestamp,
    ) -> Result<ChannelId, CreateError> {
        let result = self
            .channels
            .create(&mut self.memory, by, &name(channel), kind, now)
            .expect("stored");
        self.examine();
        result
    }

    fn join(&mut self, by: AccountId, id: ChannelId) -> Result<(), JoinError> {
        let result = self
            .channels
            .join(&mut self.memory, by, id, day(1))
            .expect("stored");
        self.examine();
        result
    }

    fn leave(&mut self, by: AccountId, id: ChannelId) -> Result<(), LeaveError> {
        let result = self
            .channels
            .leave(&mut self.memory, by, id)
            .expect("stored");
        self.examine();
        result
    }

    fn set_topic(&mut self, by: AccountId, id: ChannelId, topic: &str) -> Result<(), OwnerError> {
        let result = self
            .channels
            .set_topic(&mut self.memory, by, id, topic)
            .expect("stored");
        self.examine();
        result
    }

    fn rename(
        &mut self,
        by: AccountId,
        id: ChannelId,
        new: &str,
        now: Timestamp,
    ) -> Result<ChannelName, ChannelRenameError> {
        let result = self
            .channels
            .rename(&mut self.memory, by, id, &name(new), now)
            .expect("stored");
        self.examine();
        result
    }

    fn give(&mut self, by: AccountId, id: ChannelId, to: AccountId) -> Result<(), GiveError> {
        let result = self
            .channels
            .give(&mut self.memory, by, id, to)
            .expect("stored");
        self.examine();
        result
    }

    fn delete(&mut self, by: AccountId, id: ChannelId) -> Result<(), OwnerError> {
        let result = self
            .channels
            .delete(&mut self.memory, by, id)
            .expect("stored");
        self.examine();
        result
    }

    fn invite(
        &mut self,
        by: AccountId,
        id: ChannelId,
        account: AccountId,
    ) -> Result<(), ChannelInviteError> {
        let result = self
            .channels
            .invite(&mut self.memory, by, id, account, day(1))
            .expect("stored");
        self.examine();
        result
    }

    fn refuse(&mut self, by: AccountId, id: ChannelId) -> Result<(), RefuseError> {
        let result = self
            .channels
            .refuse(&mut self.memory, by, id)
            .expect("stored");
        self.examine();
        result
    }

    fn cancel(
        &mut self,
        by: AccountId,
        id: ChannelId,
        account: AccountId,
    ) -> Result<(), InvitationError> {
        let result = self
            .channels
            .cancel(&mut self.memory, by, id, account)
            .expect("stored");
        self.examine();
        result
    }

    fn channel(&self, id: ChannelId) -> &Channel {
        self.channels.by_id(id).expect("the channel exists")
    }

    fn members(&self, id: ChannelId) -> Vec<AccountId> {
        self.channel(id)
            .members
            .iter()
            .map(|member| member.account)
            .collect()
    }

    fn invited(&self, id: ChannelId) -> Vec<AccountId> {
        self.channel(id)
            .invited
            .iter()
            .map(|invited| invited.account)
            .collect()
    }
}

#[test]
fn starts_with_no_channels() {
    let server = Server::new();

    assert_eq!(server.channels.rows(), (&[][..], &[][..]));
    assert!(server.channels.by_name(&name("general")).is_none());
    assert!(server.channels.by_id(ChannelId(0)).is_none());
}

#[test]
fn the_member_who_makes_a_channel_is_its_owner_and_its_first_member() {
    let mut server = Server::new();

    let id = server
        .create(ANA, "general", Kind::Public, day(0))
        .expect("the name is free");

    assert_eq!(
        server.channel(id),
        &Channel {
            id,
            name: name("general"),
            kind: Kind::Public,
            owner: ANA,
            topic: String::new(),
            created: day(0),
            renamed: None,
            members: vec![Member {
                account: ANA,
                joined: day(0),
            }],
            invited: Vec::new(),
        }
    );
    assert_eq!(
        server.channels.by_name(&name("general")).map(|row| row.id),
        Some(id)
    );
    assert!(server.channel(id).is_member(ANA));
    assert!(!server.channel(id).is_member(BEN));
}

#[test]
fn gives_each_channel_a_new_number() {
    let mut server = Server::new();

    let first = server.create(ANA, "general", Kind::Public, day(0));
    let second = server.create(BEN, "dev", Kind::Private, day(0));

    assert_eq!(first, Ok(ChannelId(0)));
    assert_eq!(second, Ok(ChannelId(1)));
    server.delete(BEN, ChannelId(1)).expect("ben is the owner");
    assert_eq!(
        server.create(BEN, "dev", Kind::Private, day(0)),
        Ok(ChannelId(2))
    );
}

#[test]
fn refuses_a_channel_name_that_a_different_channel_has() {
    let (mut server, _general) = Server::with_channel(Kind::Public);

    assert_eq!(
        server.create(BEN, "general", Kind::Public, day(0)),
        Err(CreateError::NameTaken)
    );
    assert_eq!(
        server.create(BEN, "general", Kind::Private, day(0)),
        Err(CreateError::NameTaken)
    );
    assert_eq!(server.channels.rows().0.len(), 1);
}

#[test]
fn lets_a_member_own_only_the_permitted_number_of_channels() {
    let mut server = Server::new();
    for channel in ["one", "two", "three"] {
        server
            .create(ANA, channel, Kind::Public, day(0))
            .expect("in the limit");
    }

    assert_eq!(
        server.create(ANA, "four", Kind::Public, day(0)),
        Err(CreateError::TooMany)
    );

    assert_eq!(
        server.create(BEN, "four", Kind::Public, day(0)),
        Ok(ChannelId(3))
    );
    server.delete(ANA, ChannelId(0)).expect("ana is the owner");
    assert!(server.create(ANA, "five", Kind::Public, day(0)).is_ok());
}

#[test]
fn a_member_joins_a_public_channel_one_time() {
    let (mut server, general) = Server::with_channel(Kind::Public);

    assert_eq!(server.join(BEN, general), Ok(()));

    assert_eq!(server.members(general), [ANA, BEN]);
    assert_eq!(server.channel(general).members[1].joined, day(1));
    assert_eq!(server.join(BEN, general), Err(JoinError::AlreadyMember));
    assert_eq!(server.join(ANA, general), Err(JoinError::AlreadyMember));
    assert_eq!(
        server.join(BEN, ChannelId(9)),
        Err(JoinError::NoSuchChannel)
    );
}

#[test]
fn a_member_joins_a_private_channel_only_with_an_invitation() {
    let (mut server, private) = Server::with_channel(Kind::Private);

    assert_eq!(server.join(BEN, private), Err(JoinError::NotInvited));
    assert_eq!(server.invite(ANA, private, BEN), Ok(()));
    assert_eq!(server.invited(private), [BEN]);
    assert!(server.channel(private).is_invited(BEN));
    assert_eq!(server.join(CY, private), Err(JoinError::NotInvited));

    assert_eq!(server.join(BEN, private), Ok(()));

    assert_eq!(server.members(private), [ANA, BEN]);
    assert_eq!(server.invited(private), []);
    assert!(!server.channel(private).is_invited(BEN));
}

#[test]
fn only_the_owner_invites_and_only_a_person_who_is_not_in_the_channel() {
    let (mut server, private) = Server::with_channel(Kind::Private);
    server.invite(ANA, private, BEN).expect("ana is the owner");
    server.join(BEN, private).expect("ben has an invitation");

    assert_eq!(
        server.invite(BEN, private, CY),
        Err(ChannelInviteError::NotPermitted)
    );
    assert_eq!(
        server.invite(CY, private, CY),
        Err(ChannelInviteError::NotPermitted)
    );
    assert_eq!(
        server.invite(ANA, private, BEN),
        Err(ChannelInviteError::AlreadyMember)
    );
    assert_eq!(
        server.invite(ANA, private, ANA),
        Err(ChannelInviteError::AlreadyMember)
    );
    assert_eq!(server.invite(ANA, private, CY), Ok(()));
    assert_eq!(
        server.invite(ANA, private, CY),
        Err(ChannelInviteError::AlreadyInvited)
    );
    assert_eq!(
        server.invite(ANA, ChannelId(9), CY),
        Err(ChannelInviteError::NoSuchChannel)
    );
    assert_eq!(server.invited(private), [CY]);
}

#[test]
fn a_public_channel_has_no_invitations() {
    let (mut server, general) = Server::with_channel(Kind::Public);

    assert_eq!(
        server.invite(ANA, general, BEN),
        Err(ChannelInviteError::NotPrivate)
    );
    assert_eq!(server.invited(general), []);
}

#[test]
fn the_invited_member_refuses_and_the_owner_cancels_an_invitation() {
    let (mut server, private) = Server::with_channel(Kind::Private);
    server.invite(ANA, private, BEN).expect("ana is the owner");
    server.invite(ANA, private, CY).expect("ana is the owner");

    assert_eq!(server.refuse(BEN, private), Ok(()));
    assert_eq!(server.invited(private), [CY]);
    assert_eq!(server.refuse(BEN, private), Err(RefuseError::NotInvited));
    assert_eq!(
        server.refuse(CY, ChannelId(9)),
        Err(RefuseError::NoSuchChannel)
    );
    assert_eq!(server.join(BEN, private), Err(JoinError::NotInvited));

    assert_eq!(
        server.cancel(CY, private, CY),
        Err(InvitationError::NotPermitted)
    );
    assert_eq!(server.cancel(ANA, private, CY), Ok(()));
    assert_eq!(
        server.cancel(ANA, private, CY),
        Err(InvitationError::NotInvited)
    );
    assert_eq!(
        server.cancel(ANA, ChannelId(9), CY),
        Err(InvitationError::NoSuchChannel)
    );
    assert_eq!(server.invited(private), []);
}

#[test]
fn a_member_leaves_a_channel_but_the_owner_cannot() {
    let (mut server, general) = Server::with_channel(Kind::Public);
    server.join(BEN, general).expect("a public channel");
    server.join(CY, general).expect("a public channel");

    assert_eq!(server.leave(BEN, general), Ok(()));

    assert_eq!(server.members(general), [ANA, CY]);
    assert_eq!(server.leave(BEN, general), Err(LeaveError::NotMember));
    assert_eq!(server.leave(ANA, general), Err(LeaveError::Owner));
    assert_eq!(
        server.leave(CY, ChannelId(9)),
        Err(LeaveError::NoSuchChannel)
    );
    assert_eq!(server.members(general), [ANA, CY]);
    assert_eq!(server.join(BEN, general), Ok(()));
    assert_eq!(server.members(general), [ANA, CY, BEN]);
}

#[test]
fn only_the_owner_sets_the_topic() {
    let (mut server, general) = Server::with_channel(Kind::Public);
    server.join(BEN, general).expect("a public channel");

    assert_eq!(server.set_topic(ANA, general, "Plans"), Ok(()));
    assert_eq!(server.channel(general).topic, "Plans");

    assert_eq!(
        server.set_topic(BEN, general, "Mine"),
        Err(OwnerError::NotPermitted)
    );
    assert_eq!(
        server.set_topic(ANA, ChannelId(9), "x"),
        Err(OwnerError::NoSuchChannel)
    );
    assert_eq!(server.channel(general).topic, "Plans");
    assert_eq!(server.set_topic(ANA, general, ""), Ok(()));
    assert_eq!(server.channel(general).topic, "");
}

#[test]
fn the_owner_gives_the_channel_to_a_member_and_can_then_leave() {
    let (mut server, general) = Server::with_channel(Kind::Public);
    server.join(BEN, general).expect("a public channel");

    assert_eq!(server.give(ANA, general, CY), Err(GiveError::NotMember));
    assert_eq!(server.give(BEN, general, BEN), Err(GiveError::NotPermitted));
    assert_eq!(
        server.give(ANA, ChannelId(9), BEN),
        Err(GiveError::NoSuchChannel)
    );

    assert_eq!(server.give(ANA, general, BEN), Ok(()));

    assert_eq!(server.channel(general).owner, BEN);
    assert_eq!(server.members(general), [ANA, BEN]);
    assert_eq!(server.leave(BEN, general), Err(LeaveError::Owner));
    assert_eq!(server.leave(ANA, general), Ok(()));
    assert_eq!(server.members(general), [BEN]);
    assert_eq!(server.give(BEN, general, BEN), Ok(()));
}

#[test]
fn a_member_does_not_get_more_channels_than_the_limit_permits() {
    let mut server = Server::new();
    for channel in ["one", "two", "three"] {
        server
            .create(BEN, channel, Kind::Public, day(0))
            .expect("in the limit");
    }
    let general = server
        .create(ANA, "general", Kind::Public, day(0))
        .expect("the name is free");
    server.join(BEN, general).expect("a public channel");

    assert_eq!(server.give(ANA, general, BEN), Err(GiveError::TooMany));

    assert_eq!(server.channel(general).owner, ANA);
}

#[test]
fn the_owner_deletes_a_channel_and_its_name_is_free() {
    let (mut server, private) = Server::with_channel(Kind::Private);
    server.invite(ANA, private, BEN).expect("ana is the owner");
    server.join(BEN, private).expect("ben has an invitation");
    server.invite(ANA, private, CY).expect("ana is the owner");

    assert_eq!(server.delete(BEN, private), Err(OwnerError::NotPermitted));
    assert_eq!(
        server.delete(ANA, ChannelId(9)),
        Err(OwnerError::NoSuchChannel)
    );

    assert_eq!(server.delete(ANA, private), Ok(()));

    assert!(server.channels.by_id(private).is_none());
    assert!(server.channels.by_name(&name("general")).is_none());
    assert_eq!(server.delete(ANA, private), Err(OwnerError::NoSuchChannel));
    assert_eq!(
        server.create(CY, "general", Kind::Public, day(0)),
        Ok(ChannelId(1))
    );
}

#[test]
fn the_owner_renames_a_channel_and_the_old_name_stays_reserved() {
    let (mut server, general) = Server::with_channel(Kind::Public);
    server.join(BEN, general).expect("a public channel");

    assert_eq!(
        server.rename(BEN, general, "talk", day(1)),
        Err(ChannelRenameError::NotPermitted)
    );
    assert_eq!(
        server.rename(ANA, ChannelId(9), "talk", day(1)),
        Err(ChannelRenameError::NoSuchChannel)
    );

    assert_eq!(
        server.rename(ANA, general, "talk", day(1)),
        Ok(name("general"))
    );

    assert_eq!(server.channel(general).name, name("talk"));
    assert_eq!(server.channel(general).renamed, Some(day(1)));
    assert_eq!(
        server.channels.rows().1,
        [ReservedName {
            name: name("general"),
            channel: general,
            until: day(31),
        }]
    );
    assert!(server.channels.by_name(&name("general")).is_none());
    assert_eq!(
        server.create(BEN, "general", Kind::Public, day(30)),
        Err(CreateError::NameTaken)
    );
    assert_eq!(
        server.create(BEN, "general", Kind::Public, day(31)),
        Ok(ChannelId(1))
    );
    assert_eq!(server.channels.rows().1, []);
}

#[test]
fn a_channel_rename_waits_for_the_interval() {
    let (mut server, general) = Server::with_channel(Kind::Public);
    server
        .rename(ANA, general, "talk", day(1))
        .expect("the first change");

    assert_eq!(
        server.rename(ANA, general, "chat", day(30)),
        Err(ChannelRenameError::TooSoon)
    );
    assert_eq!(
        server.rename(ANA, general, "chat", day(31)),
        Ok(name("talk"))
    );
    assert_eq!(server.channel(general).name, name("chat"));
}

#[test]
fn a_channel_takes_its_own_reserved_name_back_and_no_name_of_a_different_channel() {
    let mut server = Server::new();
    let general = server
        .create(ANA, "general", Kind::Public, day(0))
        .expect("the name is free");
    let dev = server
        .create(BEN, "dev", Kind::Public, day(0))
        .expect("the name is free");
    server
        .rename(ANA, general, "talk", day(1))
        .expect("the first change");

    assert_eq!(
        server.rename(BEN, dev, "general", day(2)),
        Err(ChannelRenameError::NameTaken)
    );
    assert_eq!(
        server.rename(BEN, dev, "talk", day(2)),
        Err(ChannelRenameError::NameTaken)
    );
    assert_eq!(
        server.rename(ANA, general, "dev", day(40)),
        Err(ChannelRenameError::NameTaken)
    );

    assert_eq!(
        server.rename(ANA, general, "general", day(40)),
        Ok(name("talk"))
    );

    assert_eq!(
        server.channels.rows().1,
        [ReservedName {
            name: name("talk"),
            channel: general,
            until: day(70),
        }]
    );
    assert_eq!(server.rename(BEN, dev, "talk", day(70)), Ok(name("dev")));
}

#[test]
fn does_not_change_when_the_storage_fails() {
    let (mut server, private) = Server::with_channel(Kind::Private);
    server.invite(ANA, private, BEN).expect("ana is the owner");
    server.memory.is_broken = true;
    let rows = server.channels.rows();
    let before = Channels::load(LIMITS, server.channels.next(), rows.0, rows.1)
        .expect("the rows obey the rules");
    let memory = &mut server.memory;
    let channels = &mut server.channels;

    assert_eq!(
        channels.create(memory, BEN, &name("dev"), Kind::Public, day(1)),
        Err(Broken)
    );
    assert_eq!(channels.join(memory, BEN, private, day(1)), Err(Broken));
    assert_eq!(channels.refuse(memory, BEN, private), Err(Broken));
    assert_eq!(channels.cancel(memory, ANA, private, BEN), Err(Broken));
    assert_eq!(
        channels.invite(memory, ANA, private, CY, day(1)),
        Err(Broken)
    );
    assert_eq!(channels.set_topic(memory, ANA, private, "x"), Err(Broken));
    assert_eq!(
        channels.rename(memory, ANA, private, &name("talk"), day(1)),
        Err(Broken)
    );
    assert_eq!(channels.delete(memory, ANA, private), Err(Broken));

    assert_eq!(channels.rows(), before.rows());
    assert_eq!(channels.next(), before.next());

    server.memory.is_broken = false;
    server.join(BEN, private).expect("ben has an invitation");
    server.memory.is_broken = true;
    let memory = &mut server.memory;
    let channels = &mut server.channels;
    assert_eq!(channels.leave(memory, BEN, private), Err(Broken));
    assert_eq!(channels.give(memory, ANA, private, BEN), Err(Broken));
    assert_eq!(server.members(private), [ANA, BEN]);
    assert_eq!(server.channel(private).owner, ANA);
}

fn channel(number: u64, title: &str, owner: AccountId, members: &[AccountId]) -> Channel {
    Channel {
        id: ChannelId(number),
        name: name(title),
        kind: Kind::Private,
        owner,
        topic: String::new(),
        created: day(0),
        renamed: None,
        members: members
            .iter()
            .map(|account| Member {
                account: *account,
                joined: day(0),
            })
            .collect(),
        invited: Vec::new(),
    }
}

fn invited(accounts: &[AccountId]) -> Vec<Invited> {
    accounts
        .iter()
        .map(|account| Invited {
            account: *account,
            created: day(0),
        })
        .collect()
}

fn reserved(title: &str, number: u64) -> ReservedName {
    ReservedName {
        name: name(title),
        channel: ChannelId(number),
        until: day(30),
    }
}

#[test]
fn loads_rows_that_obey_the_rules_and_continues_the_numbers() {
    let mut second = channel(7, "dev", BEN, &[BEN, ANA]);
    second.invited = invited(&[CY]);
    let rows = [channel(2, "general", ANA, &[ANA]), second];
    let reservations = [reserved("talk", 2)];

    let mut channels =
        Channels::load(LIMITS, ChannelId(12), &rows, &reservations).expect("valid rows");

    assert_eq!(channels.rows(), (&rows[..], &reservations[..]));
    assert_eq!(channels.next(), ChannelId(12));
    let mut memory = Memory::default();
    assert_eq!(
        channels.create(&mut memory, CY, &name("new"), Kind::Public, day(0)),
        Ok(Ok(ChannelId(12)))
    );
    assert_eq!(channels.next(), ChannelId(13));
    assert_eq!(memory.next, 13);
}

#[test]
fn does_not_give_the_number_of_a_deleted_channel_to_a_new_channel_after_a_restart() {
    let mut server = Server::new();
    server
        .create(ANA, "general", Kind::Public, day(0))
        .expect("the name is free");
    let dev = server
        .create(ANA, "dev", Kind::Public, day(0))
        .expect("the name is free");
    server.delete(ANA, dev).expect("ana is the owner");
    let (rows, reservations) = server.channels.rows();

    let mut loaded = Channels::load(LIMITS, ChannelId(server.memory.next), rows, reservations)
        .expect("the rows obey the rules");

    assert_eq!(
        loaded.create(&mut server.memory, ANA, &name("talk"), Kind::Public, day(0)),
        Ok(Ok(ChannelId(2)))
    );
}

#[test]
fn tells_the_storage_the_next_number_before_the_new_channel() {
    let (mut server, general) = Server::with_channel(Kind::Public);
    server
        .rename(ANA, general, "talk", day(1))
        .expect("the first change");
    let mut log = Log::default();

    let id = server
        .channels
        .create(&mut log, BEN, &name("general"), Kind::Private, day(40))
        .expect("stored")
        .expect("the reserved time is over");

    assert_eq!(id, ChannelId(1));
    assert_eq!(
        log.changes,
        [
            ChannelChange::PutNext(ChannelId(2)),
            ChannelChange::DropReservedName(name("general")),
            ChannelChange::PutChannel(ChannelHead {
                id,
                name: name("general"),
                kind: Kind::Private,
                owner: BEN,
                topic: String::new(),
                created: day(40),
                renamed: None,
            }),
            ChannelChange::PutMember(
                id,
                Member {
                    account: BEN,
                    joined: day(40),
                }
            ),
        ]
    );
}

#[test]
fn refuses_rows_that_break_a_rule() {
    let general = || channel(1, "general", ANA, &[ANA, BEN]);
    let broken: Vec<(&str, Vec<Channel>, Vec<ReservedName>)> = vec![
        (
            "two channels with one number",
            vec![general(), channel(1, "dev", ANA, &[ANA])],
            vec![],
        ),
        (
            "two channels with one name",
            vec![general(), channel(2, "general", ANA, &[ANA])],
            vec![],
        ),
        (
            "an owner who is not a member",
            vec![channel(1, "general", CY, &[ANA, BEN])],
            vec![],
        ),
        (
            "a channel with no member",
            vec![channel(1, "general", ANA, &[])],
            vec![],
        ),
        (
            "a member two times",
            vec![channel(1, "general", ANA, &[ANA, BEN, BEN])],
            vec![],
        ),
        (
            "an invitation two times",
            vec![Channel {
                invited: invited(&[CY, CY]),
                ..general()
            }],
            vec![],
        ),
        (
            "an invitation for a member",
            vec![Channel {
                invited: invited(&[BEN]),
                ..general()
            }],
            vec![],
        ),
        (
            "an invitation in a public channel",
            vec![Channel {
                kind: Kind::Public,
                invited: invited(&[CY]),
                ..general()
            }],
            vec![],
        ),
        (
            "a reserved name that a channel has",
            vec![general()],
            vec![reserved("general", 1)],
        ),
        (
            "a reserved name two times",
            vec![general()],
            vec![reserved("talk", 1), reserved("talk", 5)],
        ),
        (
            "the largest number",
            vec![channel(u64::MAX, "general", ANA, &[ANA])],
            vec![],
        ),
        (
            "a number that is not below the next number",
            vec![channel(10, "general", ANA, &[ANA])],
            vec![],
        ),
    ];

    for (problem, channels, reservations) in broken {
        assert!(
            Channels::load(LIMITS, ChannelId(10), &channels, &reservations).is_none(),
            "{problem}"
        );
    }
}

#[test]
fn a_change_of_the_head_carries_each_field_of_the_channel() {
    let (mut server, general) = Server::with_channel(Kind::Public);
    server.set_topic(ANA, general, "Plans").expect("the owner");
    let mut log = Log::default();

    server
        .channels
        .rename(&mut log, ANA, general, &name("talk"), day(2))
        .expect("stored")
        .expect("the first change");

    assert_eq!(
        log.changes,
        [
            ChannelChange::PutReservedName(ReservedName {
                name: name("general"),
                channel: general,
                until: day(32),
            }),
            ChannelChange::PutChannel(ChannelHead {
                id: general,
                name: name("talk"),
                kind: Kind::Public,
                owner: ANA,
                topic: "Plans".to_owned(),
                created: day(0),
                renamed: Some(day(2)),
            }),
        ]
    );
}

#[derive(Default)]
struct Log {
    changes: Vec<ChannelChange>,
}

impl ChannelStorage for Log {
    type Error = Broken;

    fn write(&mut self, changes: &[ChannelChange]) -> Result<(), Broken> {
        self.changes.extend(changes.iter().cloned());
        Ok(())
    }
}

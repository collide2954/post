use post_proto::{Code, Name, Timestamp};
use post_rules::{
    Account, AccountId, Change, CodeTaken, Invitation, InviteError, Limits, RegisterError,
    RenameError, Reservation, Rules, Storage,
};

const START: u64 = 1_795_000_000_000;
const DAY: u64 = 24 * 60 * 60 * 1000;
const LIMITS: Limits = Limits {
    rename_interval: 30 * DAY,
    name_reserved: 30 * DAY,
};

#[derive(Debug, PartialEq, Eq)]
struct Broken;

#[derive(Default)]
struct Memory {
    accounts: Vec<Account>,
    invitations: Vec<Invitation>,
    reservations: Vec<Reservation>,
    is_broken: bool,
}

impl Storage for Memory {
    type Error = Broken;

    fn write(&mut self, changes: &[Change]) -> Result<(), Broken> {
        if self.is_broken {
            return Err(Broken);
        }
        for change in changes {
            match change {
                Change::PutAccount(account) => {
                    put(&mut self.accounts, account, |row| row.id == account.id);
                }
                Change::PutInvitation(invitation) => {
                    put(&mut self.invitations, invitation, |row| {
                        row.code == invitation.code
                    });
                }
                Change::PutReservation(reservation) => {
                    put(&mut self.reservations, reservation, |row| {
                        row.name == reservation.name
                    });
                }
                Change::DropReservation(name) => self.reservations.retain(|row| row.name != *name),
            }
        }
        Ok(())
    }
}

fn put<T: Clone>(rows: &mut Vec<T>, row: &T, is_same: impl Fn(&T) -> bool) {
    match rows.iter().position(is_same) {
        Some(at) => rows[at] = row.clone(),
        None => rows.push(row.clone()),
    }
}

struct Server {
    rules: Rules,
    memory: Memory,
}

impl Server {
    fn new() -> Self {
        Self {
            rules: Rules::new(LIMITS),
            memory: Memory::default(),
        }
    }

    fn with_owner() -> (Self, AccountId) {
        let mut server = Self::new();
        let link = server
            .owner_code(0, day(0))
            .expect("the server has no owner");
        let owner = server
            .register(1, "ana", &link, day(0))
            .expect("the owner link is open");
        (server, owner)
    }

    fn register(
        &mut self,
        person: u8,
        who: &str,
        invitation: &Code,
        now: Timestamp,
    ) -> Result<AccountId, RegisterError> {
        let result = self
            .rules
            .register(&mut self.memory, &key(person), &name(who), invitation, now)
            .expect("stored");
        self.examine();
        result
    }

    fn claim(&mut self, account: AccountId, invitation: &Code, now: Timestamp) -> bool {
        let claimed = self
            .rules
            .claim(&mut self.memory, account, invitation, now)
            .expect("stored");
        self.examine();
        claimed
    }

    fn owner_code(&mut self, number: u32, now: Timestamp) -> Option<Code> {
        let link = self
            .rules
            .owner_code(&mut self.memory, &code(number), now)
            .expect("stored")
            .expect("the code is new");
        self.examine();
        link
    }

    fn new_owner_code(&mut self, number: u32, now: Timestamp) -> Code {
        self.rules
            .new_owner_code(&mut self.memory, &code(number), now)
            .expect("stored")
            .expect("the code is new");
        self.examine();
        code(number)
    }

    fn rename(
        &mut self,
        account: AccountId,
        to: &str,
        now: Timestamp,
    ) -> Result<Name, RenameError> {
        let result = self
            .rules
            .rename(&mut self.memory, account, &name(to), now)
            .expect("stored");
        self.examine();
        result
    }

    fn invite(
        &mut self,
        by: AccountId,
        number: u32,
        end: Option<Timestamp>,
        uses: Option<u32>,
        now: Timestamp,
    ) -> Result<Invitation, InviteError> {
        let result = self
            .rules
            .invite(&mut self.memory, by, &code(number), end, uses, now)
            .expect("stored");
        self.examine();
        result
    }

    fn revoke(&mut self, invitation: &Code, now: Timestamp) -> bool {
        let revoked = self
            .rules
            .revoke(&mut self.memory, invitation, now)
            .expect("stored");
        self.examine();
        revoked
    }

    fn join(&mut self, owner: AccountId, person: u8, who: &str, now: Timestamp) -> AccountId {
        let invitation = self
            .invite(owner, u32::from(person) + 1000, None, None, now)
            .expect("the owner can invite");
        self.register(person, who, &invitation.code, now)
            .expect("the invitation is open")
    }

    fn role(&self, person: u8) -> Option<bool> {
        self.rules
            .account(&key(person))
            .map(|account| account.owner)
    }

    fn examine(&self) {
        let stored = (
            self.memory.accounts.as_slice(),
            self.memory.invitations.as_slice(),
            self.memory.reservations.as_slice(),
        );
        assert_eq!(self.rules.rows(), stored);

        let loaded = Rules::load(LIMITS, stored.0, stored.1, stored.2).expect("valid rows");
        assert_eq!(loaded.rows(), stored);
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

fn key(person: u8) -> [u8; 32] {
    [person; 32]
}

fn account(id: u64, person: u8, who: &str, owner: bool) -> Account {
    Account {
        id: AccountId(id),
        key: key(person),
        name: name(who),
        owner,
        created: day(0),
        renamed: None,
        invitation: code(0),
    }
}

fn invitation(number: u32, limit: Option<u32>, used: u32) -> Invitation {
    Invitation {
        code: code(number),
        owner: false,
        maker: None,
        created: day(0),
        end: None,
        uses: limit,
        used,
        revoked: None,
    }
}

fn reservation(who: &str, account: u64) -> Reservation {
    Reservation {
        name: name(who),
        account: AccountId(account),
        until: day(30),
    }
}

#[test]
fn gives_the_user_of_the_owner_link_the_owner_role() {
    let (server, owner) = Server::with_owner();

    let account = server.rules.account(&key(1)).expect("an account");
    assert_eq!(account.id, owner);
    assert_eq!(account.name, name("ana"));
    assert!(account.owner);
    assert_eq!(account.created, day(0));
    assert_eq!(server.rules.name_of(owner), Some(&name("ana")));
}

#[test]
fn does_not_know_a_key_or_an_identifier_without_an_account() {
    let (server, _owner) = Server::with_owner();

    assert_eq!(server.rules.account(&key(2)), None);
    assert_eq!(server.rules.name_of(AccountId(99)), None);
}

#[test]
fn gives_an_invited_user_the_member_role() {
    let (mut server, owner) = Server::with_owner();

    let ben = server.join(owner, 2, "ben", day(1));

    assert_ne!(ben, owner);
    assert_eq!(server.role(2), Some(false));
    assert_eq!(server.role(1), Some(true));
}

#[test]
fn refuses_a_second_account_for_one_key() {
    let (mut server, owner) = Server::with_owner();
    let invitation = server
        .invite(owner, 1, None, None, day(0))
        .expect("the owner can invite");

    let result = server.register(1, "ana-2", &invitation.code, day(0));

    assert_eq!(result, Err(RegisterError::AccountExists));
}

#[test]
fn refuses_an_invitation_that_is_not_open() {
    let (mut server, owner) = Server::with_owner();
    let revoked = server.invite(owner, 1, None, None, day(0)).expect("made");
    assert!(server.revoke(&revoked.code, day(0)));
    let ended = server
        .invite(owner, 2, Some(day(7)), None, day(0))
        .expect("made");
    let used = server
        .invite(owner, 3, None, Some(1), day(0))
        .expect("made");
    server
        .register(2, "ben", &used.code, day(0))
        .expect("the first use is valid");

    for (invitation, now) in [
        (code(99), day(0)),
        (code(0), day(0)),
        (revoked.code, day(0)),
        (ended.code.clone(), day(7)),
        (ended.code, day(8)),
        (used.code, day(0)),
    ] {
        let result = server.register(3, "cora", &invitation, now);

        assert_eq!(
            result,
            Err(RegisterError::InvalidInvitation),
            "{invitation} at {now}"
        );
    }
}

#[test]
fn accepts_an_invitation_until_its_end() {
    let (mut server, owner) = Server::with_owner();
    let invitation = server
        .invite(owner, 1, Some(day(7)), None, day(0))
        .expect("made");

    let result = server.register(2, "ben", &invitation.code, before(day(7)));

    assert!(result.is_ok());
}

#[test]
fn counts_each_use_of_an_invitation() {
    let (mut server, owner) = Server::with_owner();
    let invitation = server
        .invite(owner, 1, None, Some(2), day(0))
        .expect("made");

    assert!(server.register(2, "ben", &invitation.code, day(0)).is_ok());
    assert!(server.register(3, "cora", &invitation.code, day(0)).is_ok());
    assert_eq!(
        server.register(4, "dan", &invitation.code, day(0)),
        Err(RegisterError::InvalidInvitation)
    );
    assert!(!server.rules.code_is_open(&invitation.code, day(0)));
}

#[test]
fn refuses_a_name_that_an_account_has() {
    let (mut server, owner) = Server::with_owner();
    let invitation = server.invite(owner, 1, None, None, day(0)).expect("made");

    let result = server.register(2, "ana", &invitation.code, day(0));

    assert_eq!(result, Err(RegisterError::NameTaken));
    assert!(server.rules.code_is_open(&invitation.code, day(0)));
}

#[test]
fn gives_no_owner_link_when_the_server_has_an_owner() {
    let (mut server, _owner) = Server::with_owner();

    assert_eq!(server.owner_code(1, day(1)), None);
}

#[test]
fn gives_the_same_owner_link_until_a_person_uses_it() {
    let mut server = Server::new();

    let first = server.owner_code(1, day(0));
    let second = server.owner_code(2, day(1));

    assert_eq!(first, Some(code(1)));
    assert_eq!(second, Some(code(1)));
}

#[test]
fn refuses_an_owner_code_that_an_invitation_has() {
    let (mut server, owner) = Server::with_owner();
    server.invite(owner, 5, None, None, day(0)).expect("made");

    let result = server
        .rules
        .new_owner_code(&mut server.memory, &code(5), day(1))
        .expect("stored");

    assert_eq!(result, Err(CodeTaken));
    server.examine();
}

#[test]
fn moves_the_owner_role_to_the_new_account_of_a_new_owner_link() {
    let (mut server, _owner) = Server::with_owner();
    let link = server.new_owner_code(1, day(1));

    server.register(2, "ben", &link, day(1)).expect("open");

    assert_eq!(server.role(1), Some(false));
    assert_eq!(server.role(2), Some(true));
}

#[test]
fn moves_the_owner_role_to_an_account_that_claims_a_new_owner_link() {
    let (mut server, owner) = Server::with_owner();
    let ben = server.join(owner, 2, "ben", day(0));
    let link = server.new_owner_code(1, day(1));

    assert!(server.claim(ben, &link, day(1)));

    assert_eq!(server.role(1), Some(false));
    assert_eq!(server.role(2), Some(true));
    assert!(!server.claim(owner, &link, day(1)));
}

#[test]
fn lets_the_owner_claim_a_new_owner_link() {
    let (mut server, owner) = Server::with_owner();
    let link = server.new_owner_code(1, day(1));

    assert!(server.claim(owner, &link, day(1)));

    assert_eq!(server.role(1), Some(true));
}

#[test]
fn refuses_a_claim_that_is_not_for_an_open_owner_link() {
    let (mut server, owner) = Server::with_owner();
    let normal = server.invite(owner, 1, None, None, day(0)).expect("made");
    let link = server.new_owner_code(2, day(1));

    assert!(!server.claim(owner, &normal.code, day(1)));
    assert!(!server.claim(owner, &code(99), day(1)));
    assert!(!server.claim(AccountId(99), &link, day(1)));
    assert!(server.rules.code_is_open(&link, day(1)));
}

#[test]
fn revokes_the_earlier_owner_links_when_it_makes_a_new_one() {
    let mut server = Server::new();
    let first = server.owner_code(1, day(0)).expect("no owner");

    let second = server.new_owner_code(2, day(1));

    assert_eq!(
        server.register(1, "ana", &first, day(1)),
        Err(RegisterError::InvalidInvitation)
    );
    assert!(server.register(1, "ana", &second, day(1)).is_ok());
}

#[test]
fn changes_a_name_and_gives_the_earlier_name() {
    let (mut server, owner) = Server::with_owner();

    let earlier = server.rename(owner, "ana-2", day(1));

    assert_eq!(earlier, Ok(name("ana")));
    assert_eq!(server.rules.name_of(owner), Some(&name("ana-2")));
    let account = server.rules.account(&key(1)).expect("an account");
    assert_eq!(account.renamed, Some(day(1)));
}

#[test]
fn keeps_the_earlier_name_for_its_account_for_thirty_days() {
    let (mut server, owner) = Server::with_owner();
    let ben = server.join(owner, 2, "ben", day(0));
    server.rename(owner, "ana-2", day(1)).expect("changed");
    let invitation = server.invite(owner, 1, None, None, day(1)).expect("made");

    assert_eq!(
        server.rename(ben, "ana", before(day(31))),
        Err(RenameError::NameTaken)
    );
    assert_eq!(
        server.register(3, "ana", &invitation.code, before(day(31))),
        Err(RegisterError::NameTaken)
    );
    assert_eq!(server.rename(ben, "ana", day(31)), Ok(name("ben")));
}

#[test]
fn lets_a_new_account_take_a_name_after_its_reserved_time() {
    let (mut server, owner) = Server::with_owner();
    server.rename(owner, "ana-2", day(1)).expect("changed");
    let invitation = server.invite(owner, 1, None, None, day(1)).expect("made");

    let result = server.register(2, "ana", &invitation.code, day(31));

    assert!(result.is_ok());
}

#[test]
fn lets_an_account_take_its_earlier_name_again() {
    let (mut server, owner) = Server::with_owner();
    server.rename(owner, "ana-2", day(1)).expect("changed");

    let earlier = server.rename(owner, "ana", day(31));

    assert_eq!(earlier, Ok(name("ana-2")));
    assert_eq!(server.rules.name_of(owner), Some(&name("ana")));
}

#[test]
fn refuses_a_second_name_change_before_thirty_days() {
    let (mut server, owner) = Server::with_owner();
    server.rename(owner, "ana-2", day(1)).expect("changed");

    assert_eq!(
        server.rename(owner, "ana-3", before(day(31))),
        Err(RenameError::TooSoon)
    );
    assert_eq!(
        server.rename(owner, "ana-3", day(0)),
        Err(RenameError::TooSoon)
    );
    assert_eq!(server.rename(owner, "ana-3", day(31)), Ok(name("ana-2")));
}

#[test]
fn refuses_a_name_change_to_a_name_in_use() {
    let (mut server, owner) = Server::with_owner();
    let ben = server.join(owner, 2, "ben", day(0));

    assert_eq!(
        server.rename(ben, "ana", day(1)),
        Err(RenameError::NameTaken)
    );
    assert_eq!(
        server.rename(ben, "ben", day(1)),
        Err(RenameError::NameTaken)
    );
}

#[test]
fn refuses_a_name_change_for_an_account_that_it_does_not_know() {
    let (mut server, _owner) = Server::with_owner();

    let result = server.rename(AccountId(99), "ben", day(1));

    assert_eq!(result, Err(RenameError::UnknownAccount));
}

#[test]
fn reserves_a_name_until_the_last_time_at_the_end_of_time() {
    let (mut server, owner) = Server::with_owner();
    let last_day = Timestamp::from_unix_millis(Timestamp::MAX.unix_millis() - DAY).expect("valid");

    server.rename(owner, "ana-2", last_day).expect("changed");

    assert_eq!(server.rules.rows().2[0].until, Timestamp::MAX);
}

#[test]
fn lets_only_the_owner_make_an_invitation() {
    let (mut server, owner) = Server::with_owner();
    let ben = server.join(owner, 2, "ben", day(0));

    assert_eq!(
        server.invite(ben, 1, None, None, day(0)),
        Err(InviteError::NotPermitted)
    );
    assert_eq!(
        server.invite(AccountId(99), 1, None, None, day(0)),
        Err(InviteError::NotPermitted)
    );
    let invitation = server
        .invite(owner, 1, Some(day(7)), Some(5), day(0))
        .expect("the owner can invite");
    assert_eq!(
        invitation,
        Invitation {
            code: code(1),
            owner: false,
            maker: Some(owner),
            created: day(0),
            end: Some(day(7)),
            uses: Some(5),
            used: 0,
            revoked: None,
        }
    );
}

#[test]
fn refuses_a_code_that_an_invitation_has() {
    let (mut server, owner) = Server::with_owner();
    server.invite(owner, 1, None, None, day(0)).expect("made");

    assert_eq!(
        server.invite(owner, 1, None, None, day(0)),
        Err(InviteError::CodeTaken)
    );
    assert_eq!(
        server.invite(owner, 0, None, None, day(0)),
        Err(InviteError::CodeTaken)
    );
}

#[test]
fn revokes_only_an_open_invitation_of_a_member() {
    let (mut server, owner) = Server::with_owner();
    let invitation = server.invite(owner, 1, None, None, day(0)).expect("made");
    let link = server.new_owner_code(2, day(0));

    assert!(server.revoke(&invitation.code, day(1)));
    assert!(!server.revoke(&invitation.code, day(1)));
    assert!(!server.revoke(&link, day(1)));
    assert!(!server.revoke(&code(99), day(1)));
    assert!(server.rules.code_is_open(&link, day(1)));
}

#[test]
fn lists_the_open_invitations_of_members() {
    let (mut server, owner) = Server::with_owner();
    let open = server.invite(owner, 1, None, None, day(0)).expect("made");
    let ended = server
        .invite(owner, 2, Some(day(7)), None, day(0))
        .expect("made");
    let revoked = server.invite(owner, 3, None, None, day(0)).expect("made");
    server.revoke(&revoked.code, day(0));
    server.new_owner_code(4, day(0));

    assert_eq!(
        server.rules.open_invitations(day(1)),
        vec![open.clone(), ended]
    );
    assert_eq!(server.rules.open_invitations(day(7)), vec![open]);
}

#[test]
fn tells_if_a_code_is_open() {
    let (mut server, owner) = Server::with_owner();
    let invitation = server
        .invite(owner, 1, Some(day(7)), None, day(0))
        .expect("made");

    assert!(server.rules.code_is_open(&invitation.code, before(day(7))));
    assert!(!server.rules.code_is_open(&invitation.code, day(7)));
    assert!(!server.rules.code_is_open(&code(0), day(0)));
    assert!(!server.rules.code_is_open(&code(99), day(0)));
}

#[test]
fn changes_nothing_when_the_storage_fails() {
    let (mut server, owner) = Server::with_owner();
    let ben = server.join(owner, 2, "ben", day(0));
    let invitation = server.invite(owner, 1, None, None, day(0)).expect("made");
    let link = server.new_owner_code(2, day(0));
    server.memory.is_broken = true;
    let rules = &mut server.rules;
    let memory = &mut server.memory;

    assert_eq!(
        rules.register(memory, &key(3), &name("cora"), &invitation.code, day(1)),
        Err(Broken)
    );
    assert_eq!(rules.claim(memory, ben, &link, day(1)), Err(Broken));
    assert_eq!(
        rules.rename(memory, ben, &name("ben-2"), day(1)),
        Err(Broken)
    );
    assert_eq!(
        rules.invite(memory, owner, &code(3), None, None, day(1)),
        Err(Broken)
    );
    assert_eq!(rules.revoke(memory, &invitation.code, day(1)), Err(Broken));
    assert_eq!(rules.new_owner_code(memory, &code(4), day(1)), Err(Broken));

    server.examine();
}

#[test]
fn does_not_write_when_it_refuses_a_request() {
    let (mut server, owner) = Server::with_owner();
    server.memory.is_broken = true;
    let rules = &mut server.rules;
    let memory = &mut server.memory;

    assert_eq!(
        rules.register(memory, &key(1), &name("ana-2"), &code(0), day(1)),
        Ok(Err(RegisterError::AccountExists))
    );
    assert_eq!(rules.claim(memory, owner, &code(99), day(1)), Ok(false));
    assert_eq!(
        rules.rename(memory, owner, &name("ana"), day(1)),
        Ok(Err(RenameError::NameTaken))
    );
    assert_eq!(rules.revoke(memory, &code(99), day(1)), Ok(false));
    assert_eq!(rules.owner_code(memory, &code(5), day(1)), Ok(Ok(None)));
}

#[test]
fn fails_to_make_the_first_owner_link_when_the_storage_fails() {
    let mut server = Server::new();
    server.memory.is_broken = true;

    let result = server
        .rules
        .owner_code(&mut server.memory, &code(1), day(0));

    assert_eq!(result, Err(Broken));
    assert!(!server.rules.code_is_open(&code(1), day(0)));
}

#[test]
fn loads_rows_and_continues_after_the_highest_identifier() {
    let accounts = [account(7, 1, "ana", true), account(3, 2, "ben", false)];
    let invitations = [invitation(0, None, 2), invitation(1, None, 0)];
    let reservations = [reservation("ana-old", 7)];
    let mut memory = Memory::default();

    let mut rules =
        Rules::load(LIMITS, &accounts, &invitations, &reservations).expect("valid rows");
    let cora = rules
        .register(&mut memory, &key(3), &name("cora"), &code(1), day(1))
        .expect("stored");

    assert_eq!(cora, Ok(AccountId(8)));
    assert_eq!(
        rules.register(&mut memory, &key(4), &name("ana-old"), &code(1), day(1)),
        Ok(Err(RegisterError::NameTaken))
    );
}

#[test]
fn refuses_rows_that_break_a_rule() {
    let two = |first: Account, second: Account| Rules::load(LIMITS, &[first, second], &[], &[]);

    assert!(two(account(1, 1, "ana", true), account(2, 2, "ben", false)).is_some());
    assert!(two(account(1, 1, "ana", false), account(2, 1, "ben", false)).is_none());
    assert!(two(account(1, 1, "ana", false), account(2, 2, "ana", false)).is_none());
    assert!(two(account(1, 1, "ana", false), account(1, 2, "ben", false)).is_none());
    assert!(two(account(1, 1, "ana", true), account(2, 2, "ben", true)).is_none());
    assert!(Rules::load(LIMITS, &[account(u64::MAX, 1, "ana", false)], &[], &[]).is_none());

    let codes = |rows: &[Invitation]| Rules::load(LIMITS, &[], rows, &[]);
    assert!(codes(&[invitation(1, Some(2), 2), invitation(2, None, 9)]).is_some());
    assert!(codes(&[invitation(1, None, 0), invitation(1, None, 0)]).is_none());
    assert!(codes(&[invitation(1, Some(2), 3)]).is_none());

    let names =
        |rows: &[Reservation]| Rules::load(LIMITS, &[account(1, 1, "ana", false)], &[], rows);
    assert!(names(&[reservation("ana-old", 1), reservation("ben", 1)]).is_some());
    assert!(names(&[reservation("ana", 1)]).is_none());
    assert!(names(&[reservation("ben", 1), reservation("ben", 1)]).is_none());
}

#[test]
fn refuses_a_new_account_when_no_identifier_is_free() {
    let accounts = [account(u64::MAX - 1, 1, "ana", true)];
    let invitations = [invitation(1, None, 0)];
    let mut memory = Memory::default();
    let mut rules = Rules::load(LIMITS, &accounts, &invitations, &[]).expect("valid rows");

    let result = rules.register(&mut memory, &key(2), &name("ben"), &code(1), day(1));

    assert_eq!(result, Ok(Err(RegisterError::Full)));
}

#[test]
fn does_not_count_more_uses_than_the_number_can_hold() {
    let invitations = [invitation(1, None, u32::MAX)];
    let mut memory = Memory::default();
    let mut rules = Rules::load(LIMITS, &[], &invitations, &[]).expect("valid rows");

    let result = rules.register(&mut memory, &key(1), &name("ana"), &code(1), day(1));

    assert_eq!(result, Ok(Err(RegisterError::InvalidInvitation)));
}

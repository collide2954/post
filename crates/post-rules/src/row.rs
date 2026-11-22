use post_proto::{Code, Name, Timestamp};
use vstd::prelude::*;

verus! {

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AccountId(pub u64);

#[derive(Debug, PartialEq, Eq)]
pub struct Account {
    pub id: AccountId,
    pub key: [u8; 32],
    pub name: Name,
    pub owner: bool,
    pub created: Timestamp,
    pub renamed: Option<Timestamp>,
    pub invitation: Code,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Invitation {
    pub code: Code,
    pub owner: bool,
    pub maker: Option<AccountId>,
    pub created: Timestamp,
    pub end: Option<Timestamp>,
    pub uses: Option<u32>,
    pub used: u32,
    pub revoked: Option<Timestamp>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Reservation {
    pub name: Name,
    pub account: AccountId,
    pub until: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub rename_interval: u64,
    pub name_reserved: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterError {
    AccountExists,
    InvalidInvitation,
    NameTaken,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameError {
    UnknownAccount,
    TooSoon,
    NameTaken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteError {
    NotPermitted,
    CodeTaken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevokeError {
    NotPermitted,
    NotOpen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeTaken;

#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    PutAccount(Account),
    PutInvitation(Invitation),
    PutReservation(Reservation),
    DropReservation(Name),
}

pub trait Storage {
    type Error;

    fn write(&mut self, changes: &[Change]) -> Result<(), Self::Error>;
}

impl Account {
    pub open spec fn same(self, other: Self) -> bool {
        &&& self.id == other.id
        &&& self.key == other.key
        &&& self.name@ == other.name@
        &&& self.owner == other.owner
        &&& self.created == other.created
        &&& self.renamed == other.renamed
        &&& self.invitation@ == other.invitation@
    }
}

impl Invitation {
    pub open spec fn same(self, other: Self) -> bool {
        &&& self.code@ == other.code@
        &&& self.owner == other.owner
        &&& self.maker == other.maker
        &&& self.created == other.created
        &&& self.end == other.end
        &&& self.uses == other.uses
        &&& self.used == other.used
        &&& self.revoked == other.revoked
    }
}

impl Reservation {
    pub open spec fn same(self, other: Self) -> bool {
        &&& self.name@ == other.name@
        &&& self.account == other.account
        &&& self.until == other.until
    }
}

impl Clone for Account {
    fn clone(&self) -> (account: Self)
        ensures
            account.same(*self),
    {
        Self {
            id: self.id,
            key: self.key,
            name: self.name.clone(),
            owner: self.owner,
            created: self.created,
            renamed: self.renamed,
            invitation: self.invitation.clone(),
        }
    }
}

impl Clone for Invitation {
    fn clone(&self) -> (invitation: Self)
        ensures
            invitation.same(*self),
    {
        Self {
            code: self.code.clone(),
            owner: self.owner,
            maker: self.maker,
            created: self.created,
            end: self.end,
            uses: self.uses,
            used: self.used,
            revoked: self.revoked,
        }
    }
}

impl Clone for Reservation {
    fn clone(&self) -> (reservation: Self)
        ensures
            reservation.same(*self),
    {
        Self { name: self.name.clone(), account: self.account, until: self.until }
    }
}

} // verus!

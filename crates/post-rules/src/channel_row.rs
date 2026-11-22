use post_proto::{ChannelName, Timestamp};
use vstd::prelude::*;

use crate::row::AccountId;

verus! {

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChannelId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Public,
    Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Member {
    pub account: AccountId,
    pub joined: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Invited {
    pub account: AccountId,
    pub created: Timestamp,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Channel {
    pub id: ChannelId,
    pub name: ChannelName,
    pub kind: Kind,
    pub owner: AccountId,
    pub topic: String,
    pub created: Timestamp,
    pub renamed: Option<Timestamp>,
    pub members: Vec<Member>,
    pub invited: Vec<Invited>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ChannelHead {
    pub id: ChannelId,
    pub name: ChannelName,
    pub kind: Kind,
    pub owner: AccountId,
    pub topic: String,
    pub created: Timestamp,
    pub renamed: Option<Timestamp>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReservedName {
    pub name: ChannelName,
    pub channel: ChannelId,
    pub until: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelLimits {
    pub owned: usize,
    pub rename_interval: u64,
    pub name_reserved: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateError {
    NameTaken,
    TooMany,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinError {
    NoSuchChannel,
    AlreadyMember,
    NotInvited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveError {
    NoSuchChannel,
    NotMember,
    Owner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerError {
    NoSuchChannel,
    NotPermitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelRenameError {
    NoSuchChannel,
    NotPermitted,
    TooSoon,
    NameTaken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GiveError {
    NoSuchChannel,
    NotPermitted,
    NotMember,
    TooMany,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelInviteError {
    NoSuchChannel,
    NotPermitted,
    NotPrivate,
    AlreadyMember,
    AlreadyInvited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefuseError {
    NoSuchChannel,
    NotInvited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvitationError {
    NoSuchChannel,
    NotPermitted,
    NotInvited,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ChannelChange {
    PutNext(ChannelId),
    PutChannel(ChannelHead),
    DropChannel(ChannelId),
    PutMember(ChannelId, Member),
    DropMember(ChannelId, AccountId),
    PutInvited(ChannelId, Invited),
    DropInvited(ChannelId, AccountId),
    PutReservedName(ReservedName),
    DropReservedName(ChannelName),
}

pub trait ChannelStorage {
    type Error;

    fn write(&mut self, changes: &[ChannelChange]) -> Result<(), Self::Error>;
}

impl Kind {
    #[must_use]
    pub fn is_private(self) -> (private: bool)
        ensures
            private == (self is Private),
    {
        matches!(self, Kind::Private)
    }
}

impl Channel {
    pub open spec fn same(self, other: Self) -> bool {
        &&& self.id == other.id
        &&& self.name@ == other.name@
        &&& self.kind == other.kind
        &&& self.owner == other.owner
        &&& self.topic@ == other.topic@
        &&& self.created == other.created
        &&& self.renamed == other.renamed
        &&& self.members@ == other.members@
        &&& self.invited@ == other.invited@
    }

    #[must_use]
    pub fn head(&self) -> (head: ChannelHead)
        ensures
            head.id == self.id,
            head.name@ == self.name@,
            head.kind == self.kind,
            head.owner == self.owner,
            head.topic@ == self.topic@,
            head.created == self.created,
            head.renamed == self.renamed,
    {
        ChannelHead {
            id: self.id,
            name: self.name.clone(),
            kind: self.kind,
            owner: self.owner,
            topic: self.topic.clone(),
            created: self.created,
            renamed: self.renamed,
        }
    }
}

impl ReservedName {
    pub open spec fn same(self, other: Self) -> bool {
        &&& self.name@ == other.name@
        &&& self.channel == other.channel
        &&& self.until == other.until
    }
}

pub(crate) fn copy_of_members(members: &[Member]) -> (copy: Vec<Member>)
    ensures
        copy@ == members@,
{
    let mut copy = Vec::new();
    let mut index = 0;
    while index < members.len()
        invariant
            index <= members@.len(),
            copy@ == members@.subrange(0, index as int),
        decreases members@.len() - index,
    {
        copy.push(members[index]);
        proof {
            assert(copy@ =~= members@.subrange(0, index + 1));
        }
        index += 1;
    }
    proof {
        assert(copy@ =~= members@);
    }
    copy
}

pub(crate) fn copy_of_invited(invited: &[Invited]) -> (copy: Vec<Invited>)
    ensures
        copy@ == invited@,
{
    let mut copy = Vec::new();
    let mut index = 0;
    while index < invited.len()
        invariant
            index <= invited@.len(),
            copy@ == invited@.subrange(0, index as int),
        decreases invited@.len() - index,
    {
        copy.push(invited[index]);
        proof {
            assert(copy@ =~= invited@.subrange(0, index + 1));
        }
        index += 1;
    }
    proof {
        assert(copy@ =~= invited@);
    }
    copy
}

impl Clone for Channel {
    fn clone(&self) -> (channel: Self)
        ensures
            channel.same(*self),
    {
        Self {
            id: self.id,
            name: self.name.clone(),
            kind: self.kind,
            owner: self.owner,
            topic: self.topic.clone(),
            created: self.created,
            renamed: self.renamed,
            members: copy_of_members(self.members.as_slice()),
            invited: copy_of_invited(self.invited.as_slice()),
        }
    }
}

impl Clone for ReservedName {
    fn clone(&self) -> (reserved: Self)
        ensures
            reserved.same(*self),
    {
        Self { name: self.name.clone(), channel: self.channel, until: self.until }
    }
}

} // verus!
impl Clone for ChannelHead {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            name: self.name.clone(),
            kind: self.kind,
            owner: self.owner,
            topic: self.topic.clone(),
            created: self.created,
            renamed: self.renamed,
        }
    }
}

impl Clone for ChannelChange {
    fn clone(&self) -> Self {
        match self {
            Self::PutNext(next) => Self::PutNext(*next),
            Self::PutChannel(head) => Self::PutChannel(head.clone()),
            Self::DropChannel(id) => Self::DropChannel(*id),
            Self::PutMember(id, member) => Self::PutMember(*id, *member),
            Self::DropMember(id, account) => Self::DropMember(*id, *account),
            Self::PutInvited(id, invited) => Self::PutInvited(*id, *invited),
            Self::DropInvited(id, account) => Self::DropInvited(*id, *account),
            Self::PutReservedName(reserved) => Self::PutReservedName(reserved.clone()),
            Self::DropReservedName(name) => Self::DropReservedName(name.clone()),
        }
    }
}

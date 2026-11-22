use post_proto::{ChannelName, Timestamp};
use vstd::prelude::*;

use crate::channel_row::{
    Channel, ChannelChange, ChannelId, ChannelInviteError, ChannelLimits, ChannelRenameError,
    ChannelStorage, CreateError, GiveError, InvitationError, Invited, JoinError, Kind, LeaveError,
    Member, OwnerError, RefuseError, ReservedName, copy_of_invited, copy_of_members,
};
#[cfg(verus_keep_ghost)]
use crate::channel_rule::{
    channel_obeys, channels_obey, has_invitation, has_member, invited_are_different,
    invited_are_not_members, members_are_different, owned, people_obey,
};
use crate::row::AccountId;
#[cfg(verus_keep_ghost)]
use crate::rule::interval_is_over;
use crate::rule::{has_waited, later};

verus! {

pub struct Channels {
    list: Vec<Channel>,
    reservations: Vec<ReservedName>,
    next: u64,
    limits: ChannelLimits,
}

proof fn lemma_change_keeps_the_rules(
    channels: Seq<Channel>,
    reservations: Seq<ReservedName>,
    next: int,
    at: int,
    row: Channel,
)
    requires
        channels_obey(channels, reservations, next),
        0 <= at < channels.len(),
        row.id == channels[at].id,
        row.name@ == channels[at].name@,
        channel_obeys(row),
    ensures
        channels_obey(channels.update(at, row), reservations, next),
{
    let after = channels.update(at, row);
    assert forall|i: int| 0 <= i < after.len() implies channel_obeys(#[trigger] after[i]) by {
        if i != at {
            assert(after[i] == channels[i]);
        }
    }
    assert forall|i: int| 0 <= i < after.len() implies (#[trigger] after[i]).id.0 < next by {
        assert(after[i].id == channels[i].id);
    }
    assert forall|i: int, j: int|
        #![trigger after[i], after[j]]
        0 <= i < j < after.len() implies after[i].id != after[j].id && after[i].name@
        != after[j].name@ by {
        assert(after[i].id == channels[i].id && after[i].name@ == channels[i].name@);
        assert(after[j].id == channels[j].id && after[j].name@ == channels[j].name@);
    }
    assert forall|i: int, j: int|
        #![trigger after[i], reservations[j]]
        0 <= i < after.len() && 0 <= j < reservations.len() implies after[i].name@
        != reservations[j].name@ by {
        assert(after[i].name@ == channels[i].name@);
    }
}

proof fn lemma_removal_keeps_the_rules(
    channels: Seq<Channel>,
    reservations: Seq<ReservedName>,
    next: int,
    at: int,
)
    requires
        channels_obey(channels, reservations, next),
        0 <= at < channels.len(),
    ensures
        channels_obey(channels.remove(at), reservations, next),
        forall|i: int|
            0 <= i < channels.remove(at).len() ==> (#[trigger] channels.remove(at)[i]).id
                != channels[at].id,
{
    channels.remove_ensures(at);
    let after = channels.remove(at);
    assert forall|i: int| 0 <= i < after.len() implies channel_obeys(#[trigger] after[i]) by {
        let before = if i < at {
            i
        } else {
            i + 1
        };
        assert(after[i] == channels[before]);
    }
    assert forall|i: int| 0 <= i < after.len() implies (#[trigger] after[i]).id.0 < next
        && after[i].id != channels[at].id by {
        let before = if i < at {
            i
        } else {
            i + 1
        };
        assert(after[i] == channels[before]);
        if before < at {
            assert(channels[before].id != channels[at].id);
        } else {
            assert(channels[at].id != channels[before].id);
        }
    }
    assert forall|i: int, j: int|
        #![trigger after[i], after[j]]
        0 <= i < j < after.len() implies after[i].id != after[j].id && after[i].name@
        != after[j].name@ by {
        let first = if i < at {
            i
        } else {
            i + 1
        };
        let second = if j < at {
            j
        } else {
            j + 1
        };
        assert(after[i] == channels[first] && after[j] == channels[second]);
        assert(channels[first].id != channels[second].id);
        assert(channels[first].name@ != channels[second].name@);
    }
    assert forall|i: int, j: int|
        #![trigger after[i], reservations[j]]
        0 <= i < after.len() && 0 <= j < reservations.len() implies after[i].name@
        != reservations[j].name@ by {
        let before = if i < at {
            i
        } else {
            i + 1
        };
        assert(after[i] == channels[before]);
        assert(channels[before].name@ != reservations[j].name@);
    }
}

proof fn lemma_new_member(
    members: Seq<Member>,
    invited: Seq<Invited>,
    owner: AccountId,
    kind: Kind,
    member: Member,
)
    requires
        people_obey(members, invited, owner, kind),
        !has_member(members, member.account),
        !has_invitation(invited, member.account),
    ensures
        people_obey(members.push(member), invited, owner, kind),
        has_member(members.push(member), member.account),
{
    let after = members.push(member);
    let at = choose|i: int| 0 <= i < members.len() && (#[trigger] members[i]).account == owner;
    assert(after[at].account == owner);
    assert(after[members.len() as int].account == member.account);
    assert forall|i: int, j: int|
        #![trigger after[i], after[j]]
        0 <= i < j < after.len() implies after[i].account != after[j].account by {
        if j < members.len() {
            assert(members[i].account != members[j].account);
        } else {
            assert(members[i].account != member.account);
        }
    }
    assert forall|i: int, j: int|
        #![trigger invited[i], after[j]]
        0 <= i < invited.len() && 0 <= j < after.len() implies invited[i].account
        != after[j].account by {
        if j < members.len() {
            assert(invited[i].account != members[j].account);
        } else {
            assert(invited[i].account != member.account);
        }
    }
}

proof fn lemma_member_goes(
    members: Seq<Member>,
    invited: Seq<Invited>,
    owner: AccountId,
    kind: Kind,
    at: int,
)
    requires
        people_obey(members, invited, owner, kind),
        0 <= at < members.len(),
        members[at].account != owner,
    ensures
        people_obey(members.remove(at), invited, owner, kind),
        !has_member(members.remove(at), members[at].account),
{
    members.remove_ensures(at);
    let after = members.remove(at);
    let owner_at = choose|i: int| 0 <= i < members.len() && (#[trigger] members[i]).account == owner;
    let moved = if owner_at < at {
        owner_at
    } else {
        owner_at - 1
    };
    assert(after[moved] == members[owner_at]);
    assert(after[moved].account == owner);
    assert forall|i: int, j: int|
        #![trigger after[i], after[j]]
        0 <= i < j < after.len() implies after[i].account != after[j].account by {
        let first = if i < at {
            i
        } else {
            i + 1
        };
        let second = if j < at {
            j
        } else {
            j + 1
        };
        assert(after[i] == members[first] && after[j] == members[second]);
        assert(members[first].account != members[second].account);
    }
    assert forall|i: int, j: int|
        #![trigger invited[i], after[j]]
        0 <= i < invited.len() && 0 <= j < after.len() implies invited[i].account
        != after[j].account by {
        let before = if j < at {
            j
        } else {
            j + 1
        };
        assert(after[j] == members[before]);
        assert(invited[i].account != members[before].account);
    }
    assert forall|i: int| 0 <= i < after.len() implies (#[trigger] after[i]).account
        != members[at].account by {
        let before = if i < at {
            i
        } else {
            i + 1
        };
        assert(after[i] == members[before]);
        if before < at {
            assert(members[before].account != members[at].account);
        } else {
            assert(members[at].account != members[before].account);
        }
    }
}

proof fn lemma_new_invitation(
    members: Seq<Member>,
    invited: Seq<Invited>,
    owner: AccountId,
    kind: Kind,
    invitation: Invited,
)
    requires
        people_obey(members, invited, owner, kind),
        kind is Private,
        !has_member(members, invitation.account),
        !has_invitation(invited, invitation.account),
    ensures
        people_obey(members, invited.push(invitation), owner, kind),
        has_invitation(invited.push(invitation), invitation.account),
{
    let after = invited.push(invitation);
    assert(after[invited.len() as int].account == invitation.account);
    assert forall|i: int, j: int|
        #![trigger after[i], after[j]]
        0 <= i < j < after.len() implies after[i].account != after[j].account by {
        if j < invited.len() {
            assert(invited[i].account != invited[j].account);
        } else {
            assert(invited[i].account != invitation.account);
        }
    }
    assert forall|i: int, j: int|
        #![trigger after[i], members[j]]
        0 <= i < after.len() && 0 <= j < members.len() implies after[i].account
        != members[j].account by {
        if i < invited.len() {
            assert(invited[i].account != members[j].account);
        } else {
            assert(members[j].account != invitation.account);
        }
    }
}

proof fn lemma_invitation_goes(
    members: Seq<Member>,
    invited: Seq<Invited>,
    owner: AccountId,
    kind: Kind,
    at: int,
)
    requires
        people_obey(members, invited, owner, kind),
        0 <= at < invited.len(),
    ensures
        people_obey(members, invited.remove(at), owner, kind),
        !has_invitation(invited.remove(at), invited[at].account),
{
    invited.remove_ensures(at);
    let after = invited.remove(at);
    assert forall|i: int, j: int|
        #![trigger after[i], after[j]]
        0 <= i < j < after.len() implies after[i].account != after[j].account by {
        let first = if i < at {
            i
        } else {
            i + 1
        };
        let second = if j < at {
            j
        } else {
            j + 1
        };
        assert(after[i] == invited[first] && after[j] == invited[second]);
        assert(invited[first].account != invited[second].account);
    }
    assert forall|i: int, j: int|
        #![trigger after[i], members[j]]
        0 <= i < after.len() && 0 <= j < members.len() implies after[i].account
        != members[j].account by {
        let before = if i < at {
            i
        } else {
            i + 1
        };
        assert(after[i] == invited[before]);
        assert(invited[before].account != members[j].account);
    }
    assert forall|i: int| 0 <= i < after.len() implies (#[trigger] after[i]).account
        != invited[at].account by {
        let before = if i < at {
            i
        } else {
            i + 1
        };
        assert(after[i] == invited[before]);
        if before < at {
            assert(invited[before].account != invited[at].account);
        } else {
            assert(invited[at].account != invited[before].account);
        }
    }
}

fn member_at(members: &[Member], account: AccountId) -> (at: Option<usize>)
    ensures
        match at {
            Some(at) => at < members@.len() && members@[at as int].account == account,
            None => !has_member(members@, account),
        },
{
    let mut index = 0;
    while index < members.len()
        invariant
            index <= members@.len(),
            forall|i: int| 0 <= i < index ==> (#[trigger] members@[i]).account != account,
        decreases members@.len() - index,
    {
        if members[index].account.0 == account.0 {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn invited_at(invited: &[Invited], account: AccountId) -> (at: Option<usize>)
    ensures
        match at {
            Some(at) => at < invited@.len() && invited@[at as int].account == account,
            None => !has_invitation(invited@, account),
        },
{
    let mut index = 0;
    while index < invited.len()
        invariant
            index <= invited@.len(),
            forall|i: int| 0 <= i < index ==> (#[trigger] invited@[i]).account != account,
        decreases invited@.len() - index,
    {
        if invited[index].account.0 == account.0 {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn members_differ(members: &[Member]) -> (differ: bool)
    ensures
        differ ==> members_are_different(members@),
{
    let mut second = 0;
    while second < members.len()
        invariant
            second <= members@.len(),
            forall|i: int, j: int|
                #![trigger members@[i], members@[j]]
                0 <= i < j < second ==> members@[i].account != members@[j].account,
        decreases members@.len() - second,
    {
        let mut first = 0;
        while first < second
            invariant
                first <= second < members@.len(),
                forall|i: int|
                    0 <= i < first ==> (#[trigger] members@[i]).account
                        != members@[second as int].account,
            decreases second - first,
        {
            if members[first].account.0 == members[second].account.0 {
                return false;
            }
            first += 1;
        }
        second += 1;
    }
    true
}

fn invited_differ(invited: &[Invited]) -> (differ: bool)
    ensures
        differ ==> invited_are_different(invited@),
{
    let mut second = 0;
    while second < invited.len()
        invariant
            second <= invited@.len(),
            forall|i: int, j: int|
                #![trigger invited@[i], invited@[j]]
                0 <= i < j < second ==> invited@[i].account != invited@[j].account,
        decreases invited@.len() - second,
    {
        let mut first = 0;
        while first < second
            invariant
                first <= second < invited@.len(),
                forall|i: int|
                    0 <= i < first ==> (#[trigger] invited@[i]).account
                        != invited@[second as int].account,
            decreases second - first,
        {
            if invited[first].account.0 == invited[second].account.0 {
                return false;
            }
            first += 1;
        }
        second += 1;
    }
    true
}

fn no_invited_is_member(members: &[Member], invited: &[Invited]) -> (none: bool)
    ensures
        none ==> invited_are_not_members(members@, invited@),
{
    let mut index = 0;
    while index < invited.len()
        invariant
            index <= invited@.len(),
            forall|i: int, j: int|
                #![trigger invited@[i], members@[j]]
                0 <= i < index && 0 <= j < members@.len() ==> invited@[i].account
                    != members@[j].account,
        decreases invited@.len() - index,
    {
        if member_at(members, invited[index].account).is_some() {
            return false;
        }
        index += 1;
    }
    true
}

fn people_are_valid(channel: &Channel) -> (valid: bool)
    ensures
        valid ==> channel_obeys(*channel),
{
    let members = channel.members.as_slice();
    let invited = channel.invited.as_slice();
    member_at(members, channel.owner).is_some() && members_differ(members) && invited_differ(
        invited,
    ) && no_invited_is_member(members, invited) && (channel.kind.is_private()
        || channel.invited.is_empty())
}

fn with_people(channel: &Channel, members: Vec<Member>, invited: Vec<Invited>) -> (row: Channel)
    ensures
        row.id == channel.id,
        row.name@ == channel.name@,
        row.kind == channel.kind,
        row.owner == channel.owner,
        row.topic@ == channel.topic@,
        row.created == channel.created,
        row.renamed == channel.renamed,
        row.members@ == members@,
        row.invited@ == invited@,
{
    Channel {
        id: channel.id,
        name: channel.name.clone(),
        kind: channel.kind,
        owner: channel.owner,
        topic: channel.topic.clone(),
        created: channel.created,
        renamed: channel.renamed,
        members,
        invited,
    }
}

fn with_topic(channel: &Channel, topic: &str) -> (row: Channel)
    ensures
        row.id == channel.id,
        row.name@ == channel.name@,
        row.kind == channel.kind,
        row.owner == channel.owner,
        row.topic@ == topic@,
        row.created == channel.created,
        row.renamed == channel.renamed,
        row.members@ == channel.members@,
        row.invited@ == channel.invited@,
{
    Channel {
        id: channel.id,
        name: channel.name.clone(),
        kind: channel.kind,
        owner: channel.owner,
        topic: topic.to_owned(),
        created: channel.created,
        renamed: channel.renamed,
        members: copy_of_members(channel.members.as_slice()),
        invited: copy_of_invited(channel.invited.as_slice()),
    }
}

impl Channel {
    #[must_use]
    pub fn is_member(&self, account: AccountId) -> (member: bool)
        ensures
            member == has_member(self.members@, account),
    {
        member_at(self.members.as_slice(), account).is_some()
    }

    #[must_use]
    pub fn is_invited(&self, account: AccountId) -> (invited: bool)
        ensures
            invited == has_invitation(self.invited@, account),
    {
        invited_at(self.invited.as_slice(), account).is_some()
    }
}

impl Channels {
    #[verifier::type_invariant]
    spec fn obeys(self) -> bool {
        channels_obey(self.list@, self.reservations@, self.next as int)
    }

    pub closed spec fn channels(self) -> Seq<Channel> {
        self.list@
    }

    pub closed spec fn reservations(self) -> Seq<ReservedName> {
        self.reservations@
    }

    pub closed spec fn limits(self) -> ChannelLimits {
        self.limits
    }

    pub closed spec fn next_number(self) -> u64 {
        self.next
    }

    pub open spec fn same_as(self, other: Self) -> bool {
        &&& self.next_number() == other.next_number()
        &&& self.channels() == other.channels()
        &&& self.reservations() == other.reservations()
        &&& self.limits() == other.limits()
    }

    #[must_use]
    pub fn new(limits: ChannelLimits) -> (channels: Self)
        ensures
            channels.channels().len() == 0,
            channels.reservations().len() == 0,
            channels.limits() == limits,
            channels.next_number() == 0,
    {
        Self { list: Vec::new(), reservations: Vec::new(), next: 0, limits }
    }

    #[must_use]
    pub fn load(
        limits: ChannelLimits,
        next: ChannelId,
        channels: &[Channel],
        reservations: &[ReservedName],
    ) -> (loaded: Option<Self>)
        ensures
            loaded matches Some(loaded) ==> {
                &&& loaded.limits() == limits
                &&& loaded.next_number() == next.0
                &&& loaded.channels().len() == channels@.len()
                &&& loaded.reservations().len() == reservations@.len()
                &&& forall|i: int|
                    0 <= i < channels@.len() ==> (#[trigger] loaded.channels()[i]).same(channels@[i])
                &&& forall|i: int|
                    0 <= i < reservations@.len() ==> (#[trigger] loaded.reservations()[i]).same(
                        reservations@[i],
                    )
            },
    {
        let mut loaded = Self { list: Vec::new(), reservations: Vec::new(), next: next.0, limits };
        let mut index = 0;
        while index < channels.len()
            invariant
                index <= channels@.len(),
                loaded.limits() == limits,
                loaded.next_number() == next.0,
                loaded.channels().len() == index,
                loaded.reservations().len() == 0,
                forall|i: int| 0 <= i < index ==> (#[trigger] loaded.channels()[i]).same(channels@[i]),
            decreases channels@.len() - index,
        {
            if !loaded.admit_channel(channels[index].clone()) {
                return None;
            }
            index += 1;
        }
        let mut index = 0;
        while index < reservations.len()
            invariant
                index <= reservations@.len(),
                loaded.limits() == limits,
                loaded.next_number() == next.0,
                loaded.channels().len() == channels@.len(),
                loaded.reservations().len() == index,
                forall|i: int|
                    0 <= i < channels@.len() ==> (#[trigger] loaded.channels()[i]).same(channels@[i]),
                forall|i: int|
                    0 <= i < index ==> (#[trigger] loaded.reservations()[i]).same(reservations@[i]),
            decreases reservations@.len() - index,
        {
            if !loaded.admit_reservation(reservations[index].clone()) {
                return None;
            }
            index += 1;
        }
        Some(loaded)
    }

    #[must_use]
    pub fn next(&self) -> (next: ChannelId)
        ensures
            next.0 == self.next_number(),
    {
        ChannelId(self.next)
    }

    #[must_use]
    pub fn rows(&self) -> (rows: (&[Channel], &[ReservedName]))
        ensures
            rows.0@ == self.channels(),
            rows.1@ == self.reservations(),
    {
        (self.list.as_slice(), self.reservations.as_slice())
    }

    #[must_use]
    pub fn by_id(&self, id: ChannelId) -> (channel: Option<&Channel>)
        ensures
            match channel {
                Some(channel) => channel.id == id && self.channels().contains(*channel)
                    && channel_obeys(*channel),
                None => forall|i: int|
                    0 <= i < self.channels().len() ==> (#[trigger] self.channels()[i]).id != id,
            },
    {
        proof {
            use_type_invariant(self);
        }
        match self.id_at(id) {
            Some(at) => Some(&self.list[at]),
            None => None,
        }
    }

    #[must_use]
    pub fn by_name(&self, name: &ChannelName) -> (channel: Option<&Channel>)
        ensures
            match channel {
                Some(channel) => channel.name@ == name@ && self.channels().contains(*channel)
                    && channel_obeys(*channel),
                None => forall|i: int|
                    0 <= i < self.channels().len() ==> (#[trigger] self.channels()[i]).name@
                        != name@,
            },
    {
        proof {
            use_type_invariant(self);
        }
        match self.name_at(name) {
            Some(at) => Some(&self.list[at]),
            None => None,
        }
    }

    pub fn create<S: ChannelStorage>(
        &mut self,
        storage: &mut S,
        by: AccountId,
        name: &ChannelName,
        kind: Kind,
        now: Timestamp,
    ) -> (result: Result<Result<ChannelId, CreateError>, S::Error>)
        ensures
            !(result matches Ok(Ok(_))) ==> final(self).same_as(*old(self)),
            final(self).next_number() >= old(self).next_number(),
            result matches Ok(Ok(id)) ==> {
                &&& forall|i: int|
                    0 <= i < old(self).channels().len() ==> (#[trigger] old(self).channels()[i]).name@
                        != name@
                &&& forall|i: int|
                    0 <= i < old(self).reservations().len() && (#[trigger] old(
                        self,
                    ).reservations()[i]).name@ == name@ ==> old(self).reservations()[i].until.millis()
                        <= now.millis()
                &&& owned(old(self).channels(), by) < old(self).limits().owned
                &&& final(self).next_number() == old(self).next_number() + 1
                &&& final(self).channels().len() == old(self).channels().len() + 1
                &&& final(self).channels().last().id == id
                &&& final(self).channels().last().owner == by
                &&& final(self).channels().last().name@ == name@
                &&& final(self).channels().last().kind == kind
                &&& has_member(final(self).channels().last().members@, by)
            },
    {
        proof {
            use_type_invariant(&*self);
        }
        if self.name_at(name).is_some() {
            return Ok(Err(CreateError::NameTaken));
        }
        let reserved = self.reserved_at(name);
        let is_reserved = match reserved {
            Some(reserved) => now.unix_millis() < self.reservations[reserved].until.unix_millis(),
            None => false,
        };
        if is_reserved {
            return Ok(Err(CreateError::NameTaken));
        }
        if self.owned_by(by) >= self.limits.owned {
            return Ok(Err(CreateError::TooMany));
        }
        if self.next == u64::MAX {
            return Ok(Err(CreateError::Full));
        }
        let id = ChannelId(self.next);
        let member = Member { account: by, joined: now };
        let channel = Channel {
            id,
            name: name.clone(),
            kind,
            owner: by,
            topic: String::new(),
            created: now,
            renamed: None,
            members: vec![member],
            invited: Vec::new(),
        };
        assert(channel.members@[0].account == by);
        assert(channel_obeys(channel));
        let mut changes = vec![ChannelChange::PutNext(ChannelId(self.next + 1))];
        if reserved.is_some() {
            changes.push(ChannelChange::DropReservedName(name.clone()));
        }
        changes.push(ChannelChange::PutChannel(channel.head()));
        changes.push(ChannelChange::PutMember(id, member));
        storage.write(changes.as_slice())?;
        let (mut channels, mut reservations, next, limits) = self.take();
        if let Some(reserved) = reserved {
            reservations.remove(reserved);
        }
        channels.push(channel);
        *self = Self { list: channels, reservations, next: next + 1, limits };
        Ok(Ok(id))
    }

    pub fn join<S: ChannelStorage>(
        &mut self,
        storage: &mut S,
        by: AccountId,
        id: ChannelId,
        now: Timestamp,
    ) -> (result: Result<Result<(), JoinError>, S::Error>)
        ensures
            !(result matches Ok(Ok(_))) ==> final(self).same_as(*old(self)),
            final(self).next_number() >= old(self).next_number(),
            result matches Ok(Ok(_)) ==> exists|i: int|
                {
                    &&& 0 <= i < old(self).channels().len()
                    &&& (#[trigger] old(self).channels()[i]).id == id
                    &&& !has_member(old(self).channels()[i].members@, by)
                    &&& old(self).channels()[i].kind is Private ==> has_invitation(
                        old(self).channels()[i].invited@,
                        by,
                    )
                    &&& final(self).channels().len() == old(self).channels().len()
                    &&& has_member(final(self).channels()[i].members@, by)
                },
    {
        proof {
            use_type_invariant(&*self);
        }
        let Some(at) = self.id_at(id) else {
            return Ok(Err(JoinError::NoSuchChannel));
        };
        let channel = &self.list[at];
        if member_at(channel.members.as_slice(), by).is_some() {
            return Ok(Err(JoinError::AlreadyMember));
        }
        let invitation = invited_at(channel.invited.as_slice(), by);
        if channel.kind.is_private() && invitation.is_none() {
            return Ok(Err(JoinError::NotInvited));
        }
        let member = Member { account: by, joined: now };
        let mut members = copy_of_members(channel.members.as_slice());
        let mut invited = copy_of_invited(channel.invited.as_slice());
        if let Some(invitation) = invitation {
            proof {
                lemma_invitation_goes(
                    channel.members@,
                    channel.invited@,
                    channel.owner,
                    channel.kind,
                    invitation as int,
                );
            }
            invited.remove(invitation);
        }
        proof {
            lemma_new_member(members@, invited@, channel.owner, channel.kind, member);
        }
        members.push(member);
        let row = with_people(channel, members, invited);
        let mut changes = Vec::new();
        changes.push(ChannelChange::PutMember(id, member));
        if invitation.is_some() {
            changes.push(ChannelChange::DropInvited(id, by));
        }
        storage.write(changes.as_slice())?;
        self.put(at, row);
        assert(old(self).channels()[at as int].id == id);
        assert(self.channels()[at as int].members@ == row.members@);
        Ok(Ok(()))
    }

    pub fn leave<S: ChannelStorage>(&mut self, storage: &mut S, by: AccountId, id: ChannelId) -> (result:
        Result<Result<(), LeaveError>, S::Error>)
        ensures
            !(result matches Ok(Ok(_))) ==> final(self).same_as(*old(self)),
            final(self).next_number() >= old(self).next_number(),
            result matches Ok(Ok(_)) ==> exists|i: int|
                {
                    &&& 0 <= i < old(self).channels().len()
                    &&& (#[trigger] old(self).channels()[i]).id == id
                    &&& has_member(old(self).channels()[i].members@, by)
                    &&& old(self).channels()[i].owner != by
                    &&& final(self).channels().len() == old(self).channels().len()
                    &&& !has_member(final(self).channels()[i].members@, by)
                },
    {
        proof {
            use_type_invariant(&*self);
        }
        let Some(at) = self.id_at(id) else {
            return Ok(Err(LeaveError::NoSuchChannel));
        };
        let channel = &self.list[at];
        let Some(member) = member_at(channel.members.as_slice(), by) else {
            return Ok(Err(LeaveError::NotMember));
        };
        if channel.owner.0 == by.0 {
            return Ok(Err(LeaveError::Owner));
        }
        proof {
            lemma_member_goes(
                channel.members@,
                channel.invited@,
                channel.owner,
                channel.kind,
                member as int,
            );
        }
        let mut members = copy_of_members(channel.members.as_slice());
        members.remove(member);
        let row = with_people(channel, members, copy_of_invited(channel.invited.as_slice()));
        let changes = vec![ChannelChange::DropMember(id, by)];
        storage.write(changes.as_slice())?;
        self.put(at, row);
        assert(old(self).channels()[at as int].id == id);
        assert(old(self).channels()[at as int].members@[member as int].account == by);
        assert(self.channels()[at as int].members@ == row.members@);
        Ok(Ok(()))
    }

    pub fn set_topic<S: ChannelStorage>(
        &mut self,
        storage: &mut S,
        by: AccountId,
        id: ChannelId,
        topic: &str,
    ) -> (result: Result<Result<(), OwnerError>, S::Error>)
        ensures
            !(result matches Ok(Ok(_))) ==> final(self).same_as(*old(self)),
            final(self).next_number() >= old(self).next_number(),
            result matches Ok(Ok(_)) ==> exists|i: int|
                {
                    &&& 0 <= i < old(self).channels().len()
                    &&& (#[trigger] old(self).channels()[i]).id == id
                    &&& old(self).channels()[i].owner == by
                    &&& final(self).channels().len() == old(self).channels().len()
                    &&& final(self).channels()[i].topic@ == topic@
                },
    {
        proof {
            use_type_invariant(&*self);
        }
        let Some(at) = self.id_at(id) else {
            return Ok(Err(OwnerError::NoSuchChannel));
        };
        if self.list[at].owner.0 != by.0 {
            return Ok(Err(OwnerError::NotPermitted));
        }
        let row = with_topic(&self.list[at], topic);
        let changes = vec![ChannelChange::PutChannel(row.head())];
        storage.write(changes.as_slice())?;
        self.put(at, row);
        assert(old(self).channels()[at as int].id == id);
        assert(self.channels()[at as int].topic@ == topic@);
        Ok(Ok(()))
    }

    pub fn rename<S: ChannelStorage>(
        &mut self,
        storage: &mut S,
        by: AccountId,
        id: ChannelId,
        name: &ChannelName,
        now: Timestamp,
    ) -> (result: Result<Result<ChannelName, ChannelRenameError>, S::Error>)
        ensures
            !(result matches Ok(Ok(_))) ==> final(self).same_as(*old(self)),
            final(self).next_number() >= old(self).next_number(),
            result matches Ok(Ok(_)) ==> {
                &&& exists|i: int|
                    0 <= i < old(self).channels().len() && (#[trigger] old(self).channels()[i]).id == id
                        && old(self).channels()[i].owner == by && interval_is_over(
                        old(self).channels()[i].renamed,
                        now.millis(),
                        old(self).limits().rename_interval as int,
                    )
                &&& forall|i: int|
                    0 <= i < old(self).channels().len() ==> (#[trigger] old(self).channels()[i]).name@
                        != name@
                &&& forall|i: int|
                    0 <= i < old(self).reservations().len() && (#[trigger] old(
                        self,
                    ).reservations()[i]).name@ == name@ ==> old(self).reservations()[i].channel == id
                        || old(self).reservations()[i].until.millis() <= now.millis()
            },
    {
        proof {
            use_type_invariant(&*self);
        }
        let Some(at) = self.id_at(id) else {
            return Ok(Err(ChannelRenameError::NoSuchChannel));
        };
        if self.list[at].owner.0 != by.0 {
            return Ok(Err(ChannelRenameError::NotPermitted));
        }
        if !has_waited(self.list[at].renamed, now, self.limits.rename_interval) {
            return Ok(Err(ChannelRenameError::TooSoon));
        }
        if self.name_at(name).is_some() {
            return Ok(Err(ChannelRenameError::NameTaken));
        }
        let reserved = self.reserved_at(name);
        let is_reserved = match reserved {
            Some(reserved) => {
                let reservation = &self.reservations[reserved];
                reservation.channel.0 != id.0 && now.unix_millis() < reservation.until.unix_millis()
            },
            None => false,
        };
        if is_reserved {
            return Ok(Err(ChannelRenameError::NameTaken));
        }
        let previous = self.list[at].name.clone();
        let mut row = self.list[at].clone();
        row.name = name.clone();
        row.renamed = Some(now);
        assert(channel_obeys(row));
        let reservation = ReservedName {
            name: previous.clone(),
            channel: id,
            until: later(now, self.limits.name_reserved),
        };
        let mut changes = Vec::new();
        if reserved.is_some() {
            changes.push(ChannelChange::DropReservedName(name.clone()));
        }
        changes.push(ChannelChange::PutReservedName(reservation.clone()));
        changes.push(ChannelChange::PutChannel(row.head()));
        storage.write(changes.as_slice())?;
        let (mut channels, mut reservations, next, limits) = self.take();
        if let Some(reserved) = reserved {
            reservations.remove(reserved);
        }
        reservations.push(reservation);
        channels.set(at, row);
        assert forall|i: int| 0 <= i < channels@.len() implies channel_obeys(
            #[trigger] channels@[i],
        ) by {
            if i != at {
                assert(channels@[i] == old(self).channels()[i]);
            }
        }
        *self = Self { list: channels, reservations, next, limits };
        assert(old(self).channels()[at as int].id == id);
        Ok(Ok(previous))
    }

    pub fn give<S: ChannelStorage>(
        &mut self,
        storage: &mut S,
        by: AccountId,
        id: ChannelId,
        to: AccountId,
    ) -> (result: Result<Result<(), GiveError>, S::Error>)
        ensures
            !(result matches Ok(Ok(_))) ==> final(self).same_as(*old(self)),
            final(self).next_number() >= old(self).next_number(),
            result matches Ok(Ok(_)) ==> exists|i: int|
                {
                    &&& 0 <= i < old(self).channels().len()
                    &&& (#[trigger] old(self).channels()[i]).id == id
                    &&& old(self).channels()[i].owner == by
                    &&& has_member(old(self).channels()[i].members@, to)
                    &&& to == by || owned(old(self).channels(), to) < old(self).limits().owned
                    &&& final(self).channels().len() == old(self).channels().len()
                    &&& final(self).channels()[i].owner == to
                },
    {
        proof {
            use_type_invariant(&*self);
        }
        let Some(at) = self.id_at(id) else {
            return Ok(Err(GiveError::NoSuchChannel));
        };
        if self.list[at].owner.0 != by.0 {
            return Ok(Err(GiveError::NotPermitted));
        }
        let receiver = member_at(self.list[at].members.as_slice(), to);
        if receiver.is_none() {
            return Ok(Err(GiveError::NotMember));
        }
        assert(old(self).channels()[at as int].id == id);
        assert(old(self).channels()[at as int].members@[receiver.unwrap() as int].account == to);
        if to.0 == by.0 {
            return Ok(Ok(()));
        }
        if self.owned_by(to) >= self.limits.owned {
            return Ok(Err(GiveError::TooMany));
        }
        let mut row = self.list[at].clone();
        row.owner = to;
        assert(row.members@[receiver.unwrap() as int].account == to);
        assert(channel_obeys(row));
        let changes = vec![ChannelChange::PutChannel(row.head())];
        storage.write(changes.as_slice())?;
        self.put(at, row);
        Ok(Ok(()))
    }

    pub fn delete<S: ChannelStorage>(&mut self, storage: &mut S, by: AccountId, id: ChannelId) -> (result:
        Result<Result<(), OwnerError>, S::Error>)
        ensures
            !(result matches Ok(Ok(_))) ==> final(self).same_as(*old(self)),
            final(self).next_number() >= old(self).next_number(),
            result matches Ok(Ok(_)) ==> {
                &&& exists|i: int|
                    0 <= i < old(self).channels().len() && (#[trigger] old(self).channels()[i]).id == id
                        && old(self).channels()[i].owner == by
                &&& final(self).channels().len() == old(self).channels().len() - 1
                &&& forall|i: int|
                    0 <= i < final(self).channels().len() ==> (#[trigger] final(self).channels()[i]).id
                        != id
            },
    {
        proof {
            use_type_invariant(&*self);
        }
        let Some(at) = self.id_at(id) else {
            return Ok(Err(OwnerError::NoSuchChannel));
        };
        if self.list[at].owner.0 != by.0 {
            return Ok(Err(OwnerError::NotPermitted));
        }
        let changes = vec![ChannelChange::DropChannel(id)];
        storage.write(changes.as_slice())?;
        let (mut channels, reservations, next, limits) = self.take();
        proof {
            lemma_removal_keeps_the_rules(channels@, reservations@, next as int, at as int);
        }
        channels.remove(at);
        *self = Self { list: channels, reservations, next, limits };
        assert(old(self).channels()[at as int].id == id);
        Ok(Ok(()))
    }

    pub fn invite<S: ChannelStorage>(
        &mut self,
        storage: &mut S,
        by: AccountId,
        id: ChannelId,
        account: AccountId,
        now: Timestamp,
    ) -> (result: Result<Result<(), ChannelInviteError>, S::Error>)
        ensures
            !(result matches Ok(Ok(_))) ==> final(self).same_as(*old(self)),
            final(self).next_number() >= old(self).next_number(),
            result matches Ok(Ok(_)) ==> exists|i: int|
                {
                    &&& 0 <= i < old(self).channels().len()
                    &&& (#[trigger] old(self).channels()[i]).id == id
                    &&& old(self).channels()[i].owner == by
                    &&& old(self).channels()[i].kind is Private
                    &&& !has_member(old(self).channels()[i].members@, account)
                    &&& !has_invitation(old(self).channels()[i].invited@, account)
                    &&& final(self).channels().len() == old(self).channels().len()
                    &&& has_invitation(final(self).channels()[i].invited@, account)
                },
    {
        proof {
            use_type_invariant(&*self);
        }
        let Some(at) = self.id_at(id) else {
            return Ok(Err(ChannelInviteError::NoSuchChannel));
        };
        let channel = &self.list[at];
        if channel.owner.0 != by.0 {
            return Ok(Err(ChannelInviteError::NotPermitted));
        }
        if !channel.kind.is_private() {
            return Ok(Err(ChannelInviteError::NotPrivate));
        }
        if member_at(channel.members.as_slice(), account).is_some() {
            return Ok(Err(ChannelInviteError::AlreadyMember));
        }
        if invited_at(channel.invited.as_slice(), account).is_some() {
            return Ok(Err(ChannelInviteError::AlreadyInvited));
        }
        let invitation = Invited { account, created: now };
        proof {
            lemma_new_invitation(
                channel.members@,
                channel.invited@,
                channel.owner,
                channel.kind,
                invitation,
            );
        }
        let mut invited = copy_of_invited(channel.invited.as_slice());
        invited.push(invitation);
        let row = with_people(channel, copy_of_members(channel.members.as_slice()), invited);
        let changes = vec![ChannelChange::PutInvited(id, invitation)];
        storage.write(changes.as_slice())?;
        self.put(at, row);
        assert(old(self).channels()[at as int].id == id);
        assert(self.channels()[at as int].invited@ == row.invited@);
        Ok(Ok(()))
    }

    pub fn refuse<S: ChannelStorage>(&mut self, storage: &mut S, by: AccountId, id: ChannelId) -> (result:
        Result<Result<(), RefuseError>, S::Error>)
        ensures
            !(result matches Ok(Ok(_))) ==> final(self).same_as(*old(self)),
            final(self).next_number() >= old(self).next_number(),
            result matches Ok(Ok(_)) ==> exists|i: int|
                {
                    &&& 0 <= i < old(self).channels().len()
                    &&& (#[trigger] old(self).channels()[i]).id == id
                    &&& has_invitation(old(self).channels()[i].invited@, by)
                    &&& final(self).channels().len() == old(self).channels().len()
                    &&& !has_invitation(final(self).channels()[i].invited@, by)
                },
    {
        proof {
            use_type_invariant(&*self);
        }
        let Some(at) = self.id_at(id) else {
            return Ok(Err(RefuseError::NoSuchChannel));
        };
        let Some(invitation) = invited_at(self.list[at].invited.as_slice(), by) else {
            return Ok(Err(RefuseError::NotInvited));
        };
        self.drop_invitation(storage, at, invitation)?;
        assert(old(self).channels()[at as int].id == id);
        assert(old(self).channels()[at as int].invited@[invitation as int].account == by);
        Ok(Ok(()))
    }

    pub fn cancel<S: ChannelStorage>(
        &mut self,
        storage: &mut S,
        by: AccountId,
        id: ChannelId,
        account: AccountId,
    ) -> (result: Result<Result<(), InvitationError>, S::Error>)
        ensures
            !(result matches Ok(Ok(_))) ==> final(self).same_as(*old(self)),
            final(self).next_number() >= old(self).next_number(),
            result matches Ok(Ok(_)) ==> exists|i: int|
                {
                    &&& 0 <= i < old(self).channels().len()
                    &&& (#[trigger] old(self).channels()[i]).id == id
                    &&& old(self).channels()[i].owner == by
                    &&& has_invitation(old(self).channels()[i].invited@, account)
                    &&& final(self).channels().len() == old(self).channels().len()
                    &&& !has_invitation(final(self).channels()[i].invited@, account)
                },
    {
        proof {
            use_type_invariant(&*self);
        }
        let Some(at) = self.id_at(id) else {
            return Ok(Err(InvitationError::NoSuchChannel));
        };
        if self.list[at].owner.0 != by.0 {
            return Ok(Err(InvitationError::NotPermitted));
        }
        let Some(invitation) = invited_at(self.list[at].invited.as_slice(), account) else {
            return Ok(Err(InvitationError::NotInvited));
        };
        self.drop_invitation(storage, at, invitation)?;
        assert(old(self).channels()[at as int].id == id);
        assert(old(self).channels()[at as int].invited@[invitation as int].account == account);
        Ok(Ok(()))
    }

    fn drop_invitation<S: ChannelStorage>(&mut self, storage: &mut S, at: usize, invitation: usize) -> (result:
        Result<(), S::Error>)
        requires
            at < old(self).list@.len(),
            invitation < old(self).list@[at as int].invited@.len(),
        ensures
            result is Err ==> final(self).same_as(*old(self)),
            result is Ok ==> {
                &&& final(self).list@.len() == old(self).list@.len()
                &&& !has_invitation(
                    final(self).list@[at as int].invited@,
                    old(self).list@[at as int].invited@[invitation as int].account,
                )
                &&& final(self).reservations@ == old(self).reservations@
                &&& final(self).next == old(self).next
                &&& final(self).limits == old(self).limits
            },
    {
        proof {
            use_type_invariant(&*self);
        }
        let channel = &self.list[at];
        let account = channel.invited[invitation].account;
        proof {
            lemma_invitation_goes(
                channel.members@,
                channel.invited@,
                channel.owner,
                channel.kind,
                invitation as int,
            );
        }
        let mut invited = copy_of_invited(channel.invited.as_slice());
        invited.remove(invitation);
        let row = with_people(channel, copy_of_members(channel.members.as_slice()), invited);
        let changes = vec![ChannelChange::DropInvited(channel.id, account)];
        storage.write(changes.as_slice())?;
        self.put(at, row);
        assert(self.list@[at as int].invited@ == row.invited@);
        Ok(())
    }

    fn put(&mut self, at: usize, row: Channel)
        requires
            at < old(self).list@.len(),
            row.id == old(self).list@[at as int].id,
            row.name@ == old(self).list@[at as int].name@,
            channel_obeys(row),
        ensures
            final(self).list@ == old(self).list@.update(at as int, row),
            final(self).reservations@ == old(self).reservations@,
            final(self).next == old(self).next,
            final(self).limits == old(self).limits,
    {
        proof {
            use_type_invariant(&*self);
            lemma_change_keeps_the_rules(
                self.list@,
                self.reservations@,
                self.next as int,
                at as int,
                row,
            );
        }
        let (mut channels, reservations, next, limits) = self.take();
        channels.set(at, row);
        *self = Self { list: channels, reservations, next, limits };
    }

    fn admit_channel(&mut self, channel: Channel) -> (admitted: bool)
        ensures
            admitted ==> final(self).channels() == old(self).channels().push(channel),
            final(self).reservations() == old(self).reservations(),
            final(self).limits() == old(self).limits(),
            final(self).next_number() == old(self).next_number(),
            !admitted ==> final(self).channels() == old(self).channels(),
    {
        proof {
            use_type_invariant(&*self);
        }
        let is_new = self.id_at(channel.id).is_none() && self.name_at(&channel.name).is_none()
            && self.reserved_at(&channel.name).is_none();
        if !is_new || channel.id.0 >= self.next || !people_are_valid(&channel) {
            return false;
        }
        let (mut channels, reservations, next, limits) = self.take();
        channels.push(channel);
        *self = Self { list: channels, reservations, next, limits };
        true
    }

    fn admit_reservation(&mut self, reservation: ReservedName) -> (admitted: bool)
        ensures
            admitted ==> final(self).reservations() == old(self).reservations().push(reservation),
            final(self).channels() == old(self).channels(),
            final(self).limits() == old(self).limits(),
            final(self).next_number() == old(self).next_number(),
            !admitted ==> final(self).reservations() == old(self).reservations(),
    {
        proof {
            use_type_invariant(&*self);
        }
        if self.name_at(&reservation.name).is_some() || self.reserved_at(&reservation.name).is_some() {
            return false;
        }
        let (channels, mut reservations, next, limits) = self.take();
        reservations.push(reservation);
        *self = Self { list: channels, reservations, next, limits };
        true
    }

    fn take(&mut self) -> (parts: (Vec<Channel>, Vec<ReservedName>, u64, ChannelLimits))
        ensures
            parts.0 == old(self).list,
            parts.1 == old(self).reservations,
            parts.2 == old(self).next,
            parts.3 == old(self).limits,
    {
        let mut state = Self::new(self.limits);
        core::mem::swap(self, &mut state);
        (state.list, state.reservations, state.next, state.limits)
    }

    fn owned_by(&self, account: AccountId) -> (count: usize)
        ensures
            count == owned(self.list@, account),
    {
        let mut count: usize = 0;
        let mut index = 0;
        while index < self.list.len()
            invariant
                index <= self.list@.len(),
                count <= index,
                count == owned(self.list@.subrange(0, index as int), account),
            decreases self.list@.len() - index,
        {
            assert(self.list@.subrange(0, index + 1).drop_last() =~= self.list@.subrange(
                0,
                index as int,
            ));
            if self.list[index].owner.0 == account.0 {
                count += 1;
            }
            index += 1;
        }
        assert(self.list@.subrange(0, self.list@.len() as int) =~= self.list@);
        count
    }

    fn id_at(&self, id: ChannelId) -> (at: Option<usize>)
        ensures
            match at {
                Some(at) => at < self.list@.len() && self.list@[at as int].id == id,
                None => forall|i: int|
                    0 <= i < self.list@.len() ==> (#[trigger] self.list@[i]).id != id,
            },
    {
        let mut index = 0;
        while index < self.list.len()
            invariant
                index <= self.list@.len(),
                forall|i: int| 0 <= i < index ==> (#[trigger] self.list@[i]).id != id,
            decreases self.list@.len() - index,
        {
            if self.list[index].id.0 == id.0 {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn name_at(&self, name: &ChannelName) -> (at: Option<usize>)
        ensures
            match at {
                Some(at) => at < self.list@.len() && self.list@[at as int].name@ == name@,
                None => forall|i: int|
                    0 <= i < self.list@.len() ==> (#[trigger] self.list@[i]).name@ != name@,
            },
    {
        let mut index = 0;
        while index < self.list.len()
            invariant
                index <= self.list@.len(),
                forall|i: int| 0 <= i < index ==> (#[trigger] self.list@[i]).name@ != name@,
            decreases self.list@.len() - index,
        {
            if self.list[index].name.as_str() == name.as_str() {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn reserved_at(&self, name: &ChannelName) -> (at: Option<usize>)
        ensures
            match at {
                Some(at) => at < self.reservations@.len() && self.reservations@[at as int].name@
                    == name@,
                None => forall|i: int|
                    0 <= i < self.reservations@.len() ==> (#[trigger] self.reservations@[i]).name@
                        != name@,
            },
    {
        let mut index = 0;
        while index < self.reservations.len()
            invariant
                index <= self.reservations@.len(),
                forall|i: int| 0 <= i < index ==> (#[trigger] self.reservations@[i]).name@ != name@,
            decreases self.reservations@.len() - index,
        {
            if self.reservations[index].name.as_str() == name.as_str() {
                return Some(index);
            }
            index += 1;
        }
        None
    }
}

} // verus!

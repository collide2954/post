use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::channel_row::{Channel, Invited, Kind, Member, ReservedName};
#[cfg(verus_keep_ghost)]
use crate::row::AccountId;

verus! {

pub open spec fn has_member(members: Seq<Member>, account: AccountId) -> bool {
    exists|i: int| 0 <= i < members.len() && (#[trigger] members[i]).account == account
}

pub open spec fn has_invitation(invited: Seq<Invited>, account: AccountId) -> bool {
    exists|i: int| 0 <= i < invited.len() && (#[trigger] invited[i]).account == account
}

pub open spec fn members_are_different(members: Seq<Member>) -> bool {
    forall|i: int, j: int|
        #![trigger members[i], members[j]]
        0 <= i < j < members.len() ==> members[i].account != members[j].account
}

pub open spec fn invited_are_different(invited: Seq<Invited>) -> bool {
    forall|i: int, j: int|
        #![trigger invited[i], invited[j]]
        0 <= i < j < invited.len() ==> invited[i].account != invited[j].account
}

pub open spec fn invited_are_not_members(members: Seq<Member>, invited: Seq<Invited>) -> bool {
    forall|i: int, j: int|
        #![trigger invited[i], members[j]]
        0 <= i < invited.len() && 0 <= j < members.len() ==> invited[i].account
            != members[j].account
}

pub open spec fn people_obey(
    members: Seq<Member>,
    invited: Seq<Invited>,
    owner: AccountId,
    kind: Kind,
) -> bool {
    &&& has_member(members, owner)
    &&& members_are_different(members)
    &&& invited_are_different(invited)
    &&& invited_are_not_members(members, invited)
    &&& kind is Public ==> invited.len() == 0
}

pub open spec fn channel_obeys(channel: Channel) -> bool {
    people_obey(channel.members@, channel.invited@, channel.owner, channel.kind)
}

pub open spec fn each_channel_obeys(channels: Seq<Channel>) -> bool {
    forall|i: int| 0 <= i < channels.len() ==> channel_obeys(#[trigger] channels[i])
}

pub open spec fn channel_ids_are_different(channels: Seq<Channel>, next: int) -> bool {
    &&& forall|i: int| 0 <= i < channels.len() ==> (#[trigger] channels[i]).id.0 < next
    &&& forall|i: int, j: int|
        #![trigger channels[i], channels[j]]
        0 <= i < j < channels.len() ==> channels[i].id != channels[j].id
}

pub open spec fn channel_names_are_different(channels: Seq<Channel>) -> bool {
    forall|i: int, j: int|
        #![trigger channels[i], channels[j]]
        0 <= i < j < channels.len() ==> channels[i].name@ != channels[j].name@
}

pub open spec fn reserved_channel_names_are_free(
    channels: Seq<Channel>,
    reservations: Seq<ReservedName>,
) -> bool {
    forall|i: int, j: int|
        #![trigger channels[i], reservations[j]]
        0 <= i < channels.len() && 0 <= j < reservations.len() ==> channels[i].name@
            != reservations[j].name@
}

pub open spec fn reserved_channel_names_are_different(reservations: Seq<ReservedName>) -> bool {
    forall|i: int, j: int|
        #![trigger reservations[i], reservations[j]]
        0 <= i < j < reservations.len() ==> reservations[i].name@ != reservations[j].name@
}

pub open spec fn channels_obey(
    channels: Seq<Channel>,
    reservations: Seq<ReservedName>,
    next: int,
) -> bool {
    &&& each_channel_obeys(channels)
    &&& channel_ids_are_different(channels, next)
    &&& channel_names_are_different(channels)
    &&& reserved_channel_names_are_free(channels, reservations)
    &&& reserved_channel_names_are_different(reservations)
}

pub open spec fn owned(channels: Seq<Channel>, account: AccountId) -> nat
    decreases channels.len(),
{
    if channels.len() == 0 {
        0
    } else {
        owned(channels.drop_last(), account) + if channels.last().owner == account {
            1nat
        } else {
            0nat
        }
    }
}

} // verus!

use post_proto::Timestamp;
#[cfg(verus_keep_ghost)]
use post_proto::last_millis;
use vstd::prelude::*;

use crate::row::Invitation;
#[cfg(verus_keep_ghost)]
use crate::row::{Account, Reservation};

verus! {

pub open spec fn keys_are_different(accounts: Seq<Account>) -> bool {
    forall|i: int, j: int|
        #![trigger accounts[i], accounts[j]]
        0 <= i < j < accounts.len() ==> accounts[i].key@ != accounts[j].key@
}

pub open spec fn names_are_different(accounts: Seq<Account>) -> bool {
    forall|i: int, j: int|
        #![trigger accounts[i], accounts[j]]
        0 <= i < j < accounts.len() ==> accounts[i].name@ != accounts[j].name@
}

pub open spec fn ids_are_different(accounts: Seq<Account>, next: int) -> bool {
    &&& forall|i: int| 0 <= i < accounts.len() ==> (#[trigger] accounts[i]).id.0 < next
    &&& forall|i: int, j: int|
        #![trigger accounts[i], accounts[j]]
        0 <= i < j < accounts.len() ==> accounts[i].id != accounts[j].id
}

pub open spec fn one_owner_at_most(accounts: Seq<Account>) -> bool {
    forall|i: int, j: int|
        #![trigger accounts[i], accounts[j]]
        0 <= i < j < accounts.len() ==> !(accounts[i].owner && accounts[j].owner)
}

pub open spec fn codes_are_different(invitations: Seq<Invitation>) -> bool {
    forall|i: int, j: int|
        #![trigger invitations[i], invitations[j]]
        0 <= i < j < invitations.len() ==> invitations[i].code@ != invitations[j].code@
}

pub open spec fn uses_are_in_the_limit(invitations: Seq<Invitation>) -> bool {
    forall|i: int|
        0 <= i < invitations.len() ==> ((#[trigger] invitations[i]).uses matches Some(uses)
            ==> invitations[i].used <= uses)
}

pub open spec fn reserved_names_are_free(
    accounts: Seq<Account>,
    reservations: Seq<Reservation>,
) -> bool {
    forall|i: int, j: int|
        #![trigger accounts[i], reservations[j]]
        0 <= i < accounts.len() && 0 <= j < reservations.len() ==> accounts[i].name@
            != reservations[j].name@
}

pub open spec fn reserved_names_are_different(reservations: Seq<Reservation>) -> bool {
    forall|i: int, j: int|
        #![trigger reservations[i], reservations[j]]
        0 <= i < j < reservations.len() ==> reservations[i].name@ != reservations[j].name@
}

pub open spec fn obeys_the_rules(
    accounts: Seq<Account>,
    invitations: Seq<Invitation>,
    reservations: Seq<Reservation>,
    next: int,
) -> bool {
    &&& keys_are_different(accounts)
    &&& names_are_different(accounts)
    &&& ids_are_different(accounts, next)
    &&& one_owner_at_most(accounts)
    &&& codes_are_different(invitations)
    &&& uses_are_in_the_limit(invitations)
    &&& reserved_names_are_free(accounts, reservations)
    &&& reserved_names_are_different(reservations)
}

pub open spec fn is_open(invitation: Invitation, now: int) -> bool {
    &&& invitation.revoked is None
    &&& invitation.end matches Some(end) ==> now < end.millis()
    &&& match invitation.uses {
        Some(uses) => invitation.used < uses,
        None => invitation.used < u32::MAX,
    }
}

pub open spec fn interval_is_over(renamed: Option<Timestamp>, now: int, interval: int) -> bool {
    renamed matches Some(renamed) ==> renamed.millis() + interval <= now
}

pub fn invitation_is_open(invitation: &Invitation, now: Timestamp) -> (open: bool)
    ensures
        open == is_open(*invitation, now.millis()),
{
    let has_time = match invitation.end {
        Some(end) => now.unix_millis() < end.unix_millis(),
        None => true,
    };
    let has_uses = match invitation.uses {
        Some(uses) => invitation.used < uses,
        None => invitation.used < u32::MAX,
    };
    invitation.revoked.is_none() && has_time && has_uses
}

pub fn has_waited(renamed: Option<Timestamp>, now: Timestamp, interval: u64) -> (waited: bool)
    ensures
        waited == interval_is_over(renamed, now.millis(), interval as int),
{
    match renamed {
        Some(renamed) => renamed.unix_millis() <= now.unix_millis() && now.unix_millis()
            - renamed.unix_millis() >= interval,
        None => true,
    }
}

#[must_use]
pub fn later(time: Timestamp, millis: u64) -> (later: Timestamp)
    ensures
        later.millis() == if time.millis() + millis <= last_millis() {
            time.millis() + millis
        } else {
            last_millis()
        },
{
    match time.unix_millis().checked_add(millis) {
        Some(sum) => match Timestamp::from_unix_millis(sum) {
            Some(later) => later,
            None => Timestamp::MAX,
        },
        None => Timestamp::MAX,
    }
}

} // verus!

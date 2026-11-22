use post_proto::{Code, Name, Timestamp};
use vstd::prelude::*;

use crate::row::{
    Account, AccountId, Change, CodeTaken, Invitation, InviteError, Limits, RegisterError,
    RenameError, Reservation, RevokeError, Storage,
};
use crate::rule::{has_waited, invitation_is_open, later};
#[cfg(verus_keep_ghost)]
use crate::rule::{interval_is_over, is_open, obeys_the_rules};

verus! {

pub struct Rules {
    accounts: Vec<Account>,
    invitations: Vec<Invitation>,
    reservations: Vec<Reservation>,
    next: u64,
    limits: Limits,
}

impl Rules {
    #[verifier::type_invariant]
    spec fn obeys(self) -> bool {
        obeys_the_rules(self.accounts@, self.invitations@, self.reservations@, self.next as int)
    }

    pub closed spec fn accounts(self) -> Seq<Account> {
        self.accounts@
    }

    pub closed spec fn invitations(self) -> Seq<Invitation> {
        self.invitations@
    }

    pub closed spec fn reservations(self) -> Seq<Reservation> {
        self.reservations@
    }

    pub closed spec fn limits(self) -> Limits {
        self.limits
    }

    pub open spec fn same_as(self, other: Self) -> bool {
        &&& self.accounts() == other.accounts()
        &&& self.invitations() == other.invitations()
        &&& self.reservations() == other.reservations()
        &&& self.limits() == other.limits()
    }

    #[must_use]
    pub fn new(limits: Limits) -> (rules: Self)
        ensures
            rules.accounts().len() == 0,
            rules.invitations().len() == 0,
            rules.reservations().len() == 0,
            rules.limits() == limits,
    {
        Self {
            accounts: Vec::new(),
            invitations: Vec::new(),
            reservations: Vec::new(),
            next: 0,
            limits,
        }
    }

    #[must_use]
    pub fn load(
        limits: Limits,
        accounts: &[Account],
        invitations: &[Invitation],
        reservations: &[Reservation],
    ) -> (rules: Option<Self>)
        ensures
            rules matches Some(rules) ==> {
                &&& rules.limits() == limits
                &&& rules.accounts().len() == accounts@.len()
                &&& rules.invitations().len() == invitations@.len()
                &&& rules.reservations().len() == reservations@.len()
                &&& forall|i: int|
                    0 <= i < accounts@.len() ==> (#[trigger] rules.accounts()[i]).same(accounts@[i])
                &&& forall|i: int|
                    0 <= i < invitations@.len() ==> (#[trigger] rules.invitations()[i]).same(
                        invitations@[i],
                    )
                &&& forall|i: int|
                    0 <= i < reservations@.len() ==> (#[trigger] rules.reservations()[i]).same(
                        reservations@[i],
                    )
            },
    {
        let mut rules = Self::new(limits);
        let mut index = 0;
        while index < accounts.len()
            invariant
                index <= accounts@.len(),
                rules.limits() == limits,
                rules.accounts().len() == index,
                rules.invitations().len() == 0,
                rules.reservations().len() == 0,
                forall|i: int| 0 <= i < index ==> (#[trigger] rules.accounts()[i]).same(accounts@[i]),
            decreases accounts@.len() - index,
        {
            if !rules.admit_account(accounts[index].clone()) {
                return None;
            }
            index += 1;
        }
        let mut index = 0;
        while index < invitations.len()
            invariant
                index <= invitations@.len(),
                rules.limits() == limits,
                rules.accounts().len() == accounts@.len(),
                rules.invitations().len() == index,
                rules.reservations().len() == 0,
                forall|i: int|
                    0 <= i < accounts@.len() ==> (#[trigger] rules.accounts()[i]).same(accounts@[i]),
                forall|i: int|
                    0 <= i < index ==> (#[trigger] rules.invitations()[i]).same(invitations@[i]),
            decreases invitations@.len() - index,
        {
            if !rules.admit_invitation(invitations[index].clone()) {
                return None;
            }
            index += 1;
        }
        let mut index = 0;
        while index < reservations.len()
            invariant
                index <= reservations@.len(),
                rules.limits() == limits,
                rules.accounts().len() == accounts@.len(),
                rules.invitations().len() == invitations@.len(),
                rules.reservations().len() == index,
                forall|i: int|
                    0 <= i < accounts@.len() ==> (#[trigger] rules.accounts()[i]).same(accounts@[i]),
                forall|i: int|
                    0 <= i < invitations@.len() ==> (#[trigger] rules.invitations()[i]).same(
                        invitations@[i],
                    ),
                forall|i: int|
                    0 <= i < index ==> (#[trigger] rules.reservations()[i]).same(reservations@[i]),
            decreases reservations@.len() - index,
        {
            if !rules.admit_reservation(reservations[index].clone()) {
                return None;
            }
            index += 1;
        }
        Some(rules)
    }

    #[must_use]
    pub fn rows(&self) -> (rows: (&[Account], &[Invitation], &[Reservation]))
        ensures
            rows.0@ == self.accounts(),
            rows.1@ == self.invitations(),
            rows.2@ == self.reservations(),
    {
        (self.accounts.as_slice(), self.invitations.as_slice(), self.reservations.as_slice())
    }

    #[must_use]
    pub fn account(&self, key: &[u8; 32]) -> (account: Option<&Account>)
        ensures
            match account {
                Some(account) => account.key@ == key@ && self.accounts().contains(*account),
                None => forall|i: int|
                    0 <= i < self.accounts().len() ==> (#[trigger] self.accounts()[i]).key@ != key@,
            },
    {
        match self.key_at(key) {
            Some(at) => Some(&self.accounts[at]),
            None => None,
        }
    }

    #[must_use]
    pub fn name_of(&self, id: AccountId) -> (name: Option<&Name>)
        ensures
            match name {
                Some(name) => exists|i: int|
                    0 <= i < self.accounts().len() && (#[trigger] self.accounts()[i]).id == id
                        && self.accounts()[i].name@ == name@,
                None => forall|i: int|
                    0 <= i < self.accounts().len() ==> (#[trigger] self.accounts()[i]).id != id,
            },
    {
        match self.id_at(id) {
            Some(at) => Some(&self.accounts[at].name),
            None => None,
        }
    }

    #[must_use]
    pub fn id_of(&self, name: &Name) -> (id: Option<AccountId>)
        ensures
            match id {
                Some(id) => exists|i: int|
                    0 <= i < self.accounts().len() && (#[trigger] self.accounts()[i]).name@ == name@
                        && self.accounts()[i].id == id,
                None => forall|i: int|
                    0 <= i < self.accounts().len() ==> (#[trigger] self.accounts()[i]).name@ != name@,
            },
    {
        match self.name_at(name) {
            Some(at) => {
                assert(self.accounts()[at as int].name@ == name@);
                Some(self.accounts[at].id)
            },
            None => None,
        }
    }

    pub fn register<S: Storage>(
        &mut self,
        storage: &mut S,
        key: &[u8; 32],
        name: &Name,
        code: &Code,
        now: Timestamp,
    ) -> (result: Result<Result<Account, RegisterError>, S::Error>)
        ensures
            !(result matches Ok(Ok(_))) ==> final(self).same_as(*old(self)),
            result matches Ok(Ok(account)) ==> {
                &&& forall|i: int|
                    0 <= i < old(self).accounts().len() ==> (#[trigger] old(self).accounts()[i]).key@
                        != key@
                &&& exists|i: int|
                    0 <= i < old(self).invitations().len() && (#[trigger] old(
                        self,
                    ).invitations()[i]).code@ == code@ && is_open(
                        old(self).invitations()[i],
                        now.millis(),
                    )
                &&& forall|i: int|
                    0 <= i < old(self).reservations().len() && (#[trigger] old(
                        self,
                    ).reservations()[i]).name@ == name@ ==> old(self).reservations()[i].until.millis()
                        <= now.millis()
                &&& final(self).accounts().len() == old(self).accounts().len() + 1
                &&& final(self).accounts().last().same(account)
                &&& account.key@ == key@
                &&& account.name@ == name@
            },
    {
        proof {
            use_type_invariant(&*self);
        }
        if self.key_at(key).is_some() {
            return Ok(Err(RegisterError::AccountExists));
        }
        let Some(at) = self.code_at(code) else {
            return Ok(Err(RegisterError::InvalidInvitation));
        };
        if !invitation_is_open(&self.invitations[at], now) {
            return Ok(Err(RegisterError::InvalidInvitation));
        }
        if self.name_at(name).is_some() {
            return Ok(Err(RegisterError::NameTaken));
        }
        let reserved = self.reserved_at(name);
        let is_reserved = match reserved {
            Some(reserved) => now.unix_millis() < self.reservations[reserved].until.unix_millis(),
            None => false,
        };
        if is_reserved {
            return Ok(Err(RegisterError::NameTaken));
        }
        if self.next == u64::MAX {
            return Ok(Err(RegisterError::Full));
        }
        let owner = self.invitations[at].owner;
        let account = Account {
            id: AccountId(self.next),
            key: *key,
            name: name.clone(),
            owner,
            created: now,
            renamed: None,
            invitation: code.clone(),
        };
        let mut used = self.invitations[at].clone();
        used.used += 1;
        let previous = if owner {
            self.owner_at()
        } else {
            None
        };
        let mut changes = Vec::new();
        changes.push(Change::PutAccount(account.clone()));
        changes.push(Change::PutInvitation(used.clone()));
        if reserved.is_some() {
            changes.push(Change::DropReservation(name.clone()));
        }
        let mut member = None;
        if let Some(previous) = previous {
            let account = self.member_at(previous);
            changes.push(Change::PutAccount(account.clone()));
            member = Some(account);
        }
        storage.write(changes.as_slice())?;
        let (mut accounts, mut invitations, mut reservations, next, limits) = self.take();
        invitations.set(at, used);
        if let Some(reserved) = reserved {
            reservations.remove(reserved);
        }
        if let (Some(previous), Some(member)) = (previous, member) {
            accounts.set(previous, member);
        }
        let registered = account.clone();
        accounts.push(account);
        *self = Self { accounts, invitations, reservations, next: next + 1, limits };
        assert(is_open(old(self).invitations()[at as int], now.millis()));
        Ok(Ok(registered))
    }

    pub fn claim<S: Storage>(
        &mut self,
        storage: &mut S,
        id: AccountId,
        code: &Code,
        now: Timestamp,
    ) -> (result: Result<bool, S::Error>)
        ensures
            !(result matches Ok(true)) ==> final(self).same_as(*old(self)),
            result matches Ok(true) ==> exists|i: int|
                0 <= i < old(self).invitations().len() && (#[trigger] old(self).invitations()[i]).code@
                    == code@ && old(self).invitations()[i].owner && is_open(
                    old(self).invitations()[i],
                    now.millis(),
                ),
    {
        proof {
            use_type_invariant(&*self);
        }
        let Some(at) = self.code_at(code) else {
            return Ok(false);
        };
        if !self.invitations[at].owner || !invitation_is_open(&self.invitations[at], now) {
            return Ok(false);
        }
        let Some(claimer) = self.id_at(id) else {
            return Ok(false);
        };
        let previous = match self.owner_at() {
            Some(previous) if previous != claimer => Some(previous),
            _ => None,
        };
        let mut used = self.invitations[at].clone();
        used.used += 1;
        let mut owner = self.accounts[claimer].clone();
        owner.owner = true;
        let mut changes = Vec::new();
        changes.push(Change::PutInvitation(used.clone()));
        changes.push(Change::PutAccount(owner.clone()));
        let mut member = None;
        if let Some(previous) = previous {
            let account = self.member_at(previous);
            changes.push(Change::PutAccount(account.clone()));
            member = Some(account);
        }
        storage.write(changes.as_slice())?;
        let (mut accounts, mut invitations, reservations, next, limits) = self.take();
        invitations.set(at, used);
        if let (Some(previous), Some(member)) = (previous, member) {
            accounts.set(previous, member);
        }
        accounts.set(claimer, owner);
        *self = Self { accounts, invitations, reservations, next, limits };
        assert(is_open(old(self).invitations()[at as int], now.millis()));
        Ok(true)
    }

    pub fn owner_code<S: Storage>(&mut self, storage: &mut S, fresh: &Code, now: Timestamp) -> (result:
        Result<Result<Option<Code>, CodeTaken>, S::Error>)
        ensures
            result matches Ok(Ok(None)) ==> final(self).same_as(*old(self)) && exists|i: int|
                0 <= i < old(self).accounts().len() && (#[trigger] old(self).accounts()[i]).owner,
            result matches Ok(Err(_)) ==> final(self).same_as(*old(self)),
            result matches Err(_) ==> final(self).same_as(*old(self)),
    {
        proof {
            use_type_invariant(&*self);
        }
        let owner = self.owner_at();
        if owner.is_some() {
            assert(old(self).accounts()[owner.unwrap() as int].owner);
            return Ok(Ok(None));
        }
        if let Some(waiting) = self.open_owner_code_at(now) {
            return Ok(Ok(Some(self.invitations[waiting].code.clone())));
        }
        match self.add(storage, owner_invitation(fresh, now)) {
            Ok(Ok(())) => Ok(Ok(Some(fresh.clone()))),
            Ok(Err(taken)) => Ok(Err(taken)),
            Err(error) => Err(error),
        }
    }

    pub fn new_owner_code<S: Storage>(&mut self, storage: &mut S, fresh: &Code, now: Timestamp) -> (result:
        Result<Result<(), CodeTaken>, S::Error>)
        ensures
            !(result matches Ok(Ok(_))) ==> final(self).same_as(*old(self)),
    {
        proof {
            use_type_invariant(&*self);
        }
        if self.code_at(fresh).is_some() {
            return Ok(Err(CodeTaken));
        }
        let fresh = owner_invitation(fresh, now);
        let mut changes = Vec::new();
        let mut index = 0;
        while index < self.invitations.len()
            invariant
                index <= self.invitations@.len(),
            decreases self.invitations@.len() - index,
        {
            if self.invitations[index].owner && self.invitations[index].revoked.is_none() {
                let mut revoked = self.invitations[index].clone();
                revoked.revoked = Some(now);
                changes.push(Change::PutInvitation(revoked));
            }
            index += 1;
        }
        changes.push(Change::PutInvitation(fresh.clone()));
        storage.write(changes.as_slice())?;
        let (accounts, mut invitations, reservations, next, limits) = self.take();
        let ghost before = invitations@;
        let mut index = 0;
        while index < invitations.len()
            invariant
                index <= invitations@.len(),
                invitations@.len() == before.len(),
                forall|i: int|
                    0 <= i < before.len() ==> {
                        &&& (#[trigger] invitations@[i]).code@ == before[i].code@
                        &&& invitations@[i].uses == before[i].uses
                        &&& invitations@[i].used == before[i].used
                    },
            decreases invitations@.len() - index,
        {
            if invitations[index].owner && invitations[index].revoked.is_none() {
                let mut revoked = invitations[index].clone();
                revoked.revoked = Some(now);
                invitations.set(index, revoked);
            }
            index += 1;
        }
        invitations.push(fresh);
        *self = Self { accounts, invitations, reservations, next, limits };
        Ok(Ok(()))
    }

    pub fn rename<S: Storage>(
        &mut self,
        storage: &mut S,
        id: AccountId,
        name: &Name,
        now: Timestamp,
    ) -> (result: Result<Result<Name, RenameError>, S::Error>)
        ensures
            !(result matches Ok(Ok(_))) ==> final(self).same_as(*old(self)),
            result matches Ok(Ok(_)) ==> {
                &&& exists|i: int|
                    0 <= i < old(self).accounts().len() && (#[trigger] old(self).accounts()[i]).id == id
                        && interval_is_over(
                        old(self).accounts()[i].renamed,
                        now.millis(),
                        old(self).limits().rename_interval as int,
                    )
                &&& forall|i: int|
                    0 <= i < old(self).accounts().len() ==> (#[trigger] old(self).accounts()[i]).name@
                        != name@
                &&& forall|i: int|
                    0 <= i < old(self).reservations().len() && (#[trigger] old(
                        self,
                    ).reservations()[i]).name@ == name@ ==> old(self).reservations()[i].account == id
                        || old(self).reservations()[i].until.millis() <= now.millis()
            },
    {
        proof {
            use_type_invariant(&*self);
        }
        let Some(at) = self.id_at(id) else {
            return Ok(Err(RenameError::UnknownAccount));
        };
        if !has_waited(self.accounts[at].renamed, now, self.limits.rename_interval) {
            return Ok(Err(RenameError::TooSoon));
        }
        if self.name_at(name).is_some() {
            return Ok(Err(RenameError::NameTaken));
        }
        let reserved = self.reserved_at(name);
        let is_reserved = match reserved {
            Some(reserved) => {
                let reservation = &self.reservations[reserved];
                reservation.account.0 != id.0 && now.unix_millis() < reservation.until.unix_millis()
            },
            None => false,
        };
        if is_reserved {
            return Ok(Err(RenameError::NameTaken));
        }
        let old = self.accounts[at].name.clone();
        let mut renamed = self.accounts[at].clone();
        renamed.name = name.clone();
        renamed.renamed = Some(now);
        let reservation = Reservation {
            name: old.clone(),
            account: id,
            until: later(now, self.limits.name_reserved),
        };
        let mut changes = Vec::new();
        if reserved.is_some() {
            changes.push(Change::DropReservation(name.clone()));
        }
        changes.push(Change::PutReservation(reservation.clone()));
        changes.push(Change::PutAccount(renamed.clone()));
        storage.write(changes.as_slice())?;
        let (mut accounts, invitations, mut reservations, next, limits) = self.take();
        if let Some(reserved) = reserved {
            reservations.remove(reserved);
        }
        reservations.push(reservation);
        accounts.set(at, renamed);
        *self = Self { accounts, invitations, reservations, next, limits };
        Ok(Ok(old))
    }

    pub fn invite<S: Storage>(
        &mut self,
        storage: &mut S,
        by: AccountId,
        code: &Code,
        end: Option<Timestamp>,
        uses: Option<u32>,
        now: Timestamp,
    ) -> (result: Result<Result<Invitation, InviteError>, S::Error>)
        ensures
            !(result matches Ok(Ok(_))) ==> final(self).same_as(*old(self)),
            result matches Ok(Ok(_)) ==> exists|i: int|
                0 <= i < old(self).accounts().len() && (#[trigger] old(self).accounts()[i]).id == by
                    && old(self).accounts()[i].owner,
    {
        proof {
            use_type_invariant(&*self);
        }
        if !self.is_owner(by) {
            return Ok(Err(InviteError::NotPermitted));
        }
        let invitation = Invitation {
            code: code.clone(),
            owner: false,
            maker: Some(by),
            created: now,
            end,
            uses,
            used: 0,
            revoked: None,
        };
        match self.add(storage, invitation.clone()) {
            Ok(Ok(())) => Ok(Ok(invitation)),
            Ok(Err(_)) => Ok(Err(InviteError::CodeTaken)),
            Err(error) => Err(error),
        }
    }

    pub fn revoke<S: Storage>(
        &mut self,
        storage: &mut S,
        by: AccountId,
        code: &Code,
        now: Timestamp,
    ) -> (result: Result<Result<(), RevokeError>, S::Error>)
        ensures
            !(result matches Ok(Ok(_))) ==> final(self).same_as(*old(self)),
            result matches Ok(Ok(_)) ==> {
                &&& exists|i: int|
                    0 <= i < old(self).accounts().len() && (#[trigger] old(self).accounts()[i]).id == by
                        && old(self).accounts()[i].owner
                &&& exists|i: int|
                    0 <= i < old(self).invitations().len() && (#[trigger] old(
                        self,
                    ).invitations()[i]).code@ == code@ && !old(self).invitations()[i].owner && is_open(
                        old(self).invitations()[i],
                        now.millis(),
                    )
            },
    {
        proof {
            use_type_invariant(&*self);
        }
        if !self.is_owner(by) {
            return Ok(Err(RevokeError::NotPermitted));
        }
        let Some(at) = self.code_at(code) else {
            return Ok(Err(RevokeError::NotOpen));
        };
        if self.invitations[at].owner || !invitation_is_open(&self.invitations[at], now) {
            return Ok(Err(RevokeError::NotOpen));
        }
        let mut revoked = self.invitations[at].clone();
        revoked.revoked = Some(now);
        let changes = vec![Change::PutInvitation(revoked.clone())];
        storage.write(changes.as_slice())?;
        let (accounts, mut invitations, reservations, next, limits) = self.take();
        invitations.set(at, revoked);
        *self = Self { accounts, invitations, reservations, next, limits };
        assert(is_open(old(self).invitations()[at as int], now.millis()));
        Ok(Ok(()))
    }

    #[must_use]
    pub fn is_owner(&self, id: AccountId) -> (owner: bool)
        ensures
            owner ==> exists|i: int|
                0 <= i < self.accounts().len() && (#[trigger] self.accounts()[i]).id == id
                    && self.accounts()[i].owner,
    {
        match self.id_at(id) {
            Some(at) => {
                let owner = self.accounts[at].owner;
                assert(owner ==> self.accounts()[at as int].id == id);
                owner
            },
            None => false,
        }
    }

    #[must_use]
    pub fn code_is_open(&self, code: &Code, now: Timestamp) -> (open: bool)
        ensures
            open == exists|i: int|
                0 <= i < self.invitations().len() && (#[trigger] self.invitations()[i]).code@ == code@
                    && is_open(self.invitations()[i], now.millis()),
    {
        proof {
            use_type_invariant(self);
        }
        match self.code_at(code) {
            Some(at) => {
                let open = invitation_is_open(&self.invitations[at], now);
                assert(open ==> self.invitations()[at as int].code@ == code@);
                open
            },
            None => false,
        }
    }

    #[must_use]
    pub fn open_invitations(&self, now: Timestamp) -> (open: Vec<Invitation>)
        ensures
            forall|i: int|
                0 <= i < open@.len() ==> !(#[trigger] open@[i]).owner && is_open(open@[i], now.millis()),
    {
        let mut open: Vec<Invitation> = Vec::new();
        let mut index = 0;
        while index < self.invitations.len()
            invariant
                index <= self.invitations@.len(),
                forall|i: int|
                    0 <= i < open@.len() ==> !(#[trigger] open@[i]).owner && is_open(
                        open@[i],
                        now.millis(),
                    ),
            decreases self.invitations@.len() - index,
        {
            let invitation = &self.invitations[index];
            if !invitation.owner && invitation_is_open(invitation, now) {
                open.push(invitation.clone());
            }
            index += 1;
        }
        open
    }

    fn add<S: Storage>(&mut self, storage: &mut S, invitation: Invitation) -> (result: Result<
        Result<(), CodeTaken>,
        S::Error,
    >)
        requires
            invitation.used == 0,
        ensures
            !(result matches Ok(Ok(_))) ==> final(self).same_as(*old(self)),
            result matches Ok(Ok(_)) ==> {
                &&& final(self).accounts() == old(self).accounts()
                &&& final(self).reservations() == old(self).reservations()
                &&& final(self).limits() == old(self).limits()
            },
    {
        proof {
            use_type_invariant(&*self);
        }
        if self.code_at(&invitation.code).is_some() {
            return Ok(Err(CodeTaken));
        }
        let changes = vec![Change::PutInvitation(invitation.clone())];
        storage.write(changes.as_slice())?;
        let (accounts, mut invitations, reservations, next, limits) = self.take();
        invitations.push(invitation);
        *self = Self { accounts, invitations, reservations, next, limits };
        Ok(Ok(()))
    }

    fn admit_account(&mut self, account: Account) -> (admitted: bool)
        ensures
            admitted ==> final(self).accounts() == old(self).accounts().push(account),
            final(self).invitations() == old(self).invitations(),
            final(self).reservations() == old(self).reservations(),
            final(self).limits() == old(self).limits(),
            !admitted ==> final(self).accounts() == old(self).accounts(),
    {
        proof {
            use_type_invariant(&*self);
        }
        let is_new = self.key_at(&account.key).is_none() && self.name_at(&account.name).is_none()
            && self.id_at(account.id).is_none() && self.reserved_at(&account.name).is_none();
        let is_one_owner = !account.owner || self.owner_at().is_none();
        if !is_new || !is_one_owner || account.id.0 == u64::MAX {
            return false;
        }
        let (mut accounts, invitations, reservations, next, limits) = self.take();
        let next = if account.id.0 < next {
            next
        } else {
            account.id.0 + 1
        };
        accounts.push(account);
        *self = Self { accounts, invitations, reservations, next, limits };
        true
    }

    fn admit_invitation(&mut self, invitation: Invitation) -> (admitted: bool)
        ensures
            admitted ==> final(self).invitations() == old(self).invitations().push(invitation),
            final(self).accounts() == old(self).accounts(),
            final(self).reservations() == old(self).reservations(),
            final(self).limits() == old(self).limits(),
            !admitted ==> final(self).invitations() == old(self).invitations(),
    {
        proof {
            use_type_invariant(&*self);
        }
        let is_in_the_limit = match invitation.uses {
            Some(uses) => invitation.used <= uses,
            None => true,
        };
        if !is_in_the_limit || self.code_at(&invitation.code).is_some() {
            return false;
        }
        let (accounts, mut invitations, reservations, next, limits) = self.take();
        invitations.push(invitation);
        *self = Self { accounts, invitations, reservations, next, limits };
        true
    }

    fn admit_reservation(&mut self, reservation: Reservation) -> (admitted: bool)
        ensures
            admitted ==> final(self).reservations() == old(self).reservations().push(reservation),
            final(self).accounts() == old(self).accounts(),
            final(self).invitations() == old(self).invitations(),
            final(self).limits() == old(self).limits(),
            !admitted ==> final(self).reservations() == old(self).reservations(),
    {
        proof {
            use_type_invariant(&*self);
        }
        if self.name_at(&reservation.name).is_some() || self.reserved_at(&reservation.name).is_some() {
            return false;
        }
        let (accounts, invitations, mut reservations, next, limits) = self.take();
        reservations.push(reservation);
        *self = Self { accounts, invitations, reservations, next, limits };
        true
    }

    fn take(&mut self) -> (parts: (Vec<Account>, Vec<Invitation>, Vec<Reservation>, u64, Limits))
        ensures
            parts.0 == old(self).accounts,
            parts.1 == old(self).invitations,
            parts.2 == old(self).reservations,
            parts.3 == old(self).next,
            parts.4 == old(self).limits,
    {
        let mut state = Self::new(self.limits);
        core::mem::swap(self, &mut state);
        (state.accounts, state.invitations, state.reservations, state.next, state.limits)
    }

    fn member_at(&self, at: usize) -> (member: Account)
        requires
            at < self.accounts@.len(),
        ensures
            member.id == self.accounts@[at as int].id,
            member.key == self.accounts@[at as int].key,
            member.name@ == self.accounts@[at as int].name@,
            !member.owner,
    {
        let mut member = self.accounts[at].clone();
        member.owner = false;
        member
    }

    fn key_at(&self, key: &[u8; 32]) -> (at: Option<usize>)
        ensures
            match at {
                Some(at) => at < self.accounts@.len() && self.accounts@[at as int].key@ == key@,
                None => forall|i: int|
                    0 <= i < self.accounts@.len() ==> (#[trigger] self.accounts@[i]).key@ != key@,
            },
    {
        let mut index = 0;
        while index < self.accounts.len()
            invariant
                index <= self.accounts@.len(),
                forall|i: int| 0 <= i < index ==> (#[trigger] self.accounts@[i]).key@ != key@,
            decreases self.accounts@.len() - index,
        {
            if self.accounts[index].key == *key {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn id_at(&self, id: AccountId) -> (at: Option<usize>)
        ensures
            match at {
                Some(at) => at < self.accounts@.len() && self.accounts@[at as int].id == id,
                None => forall|i: int|
                    0 <= i < self.accounts@.len() ==> (#[trigger] self.accounts@[i]).id != id,
            },
    {
        let mut index = 0;
        while index < self.accounts.len()
            invariant
                index <= self.accounts@.len(),
                forall|i: int| 0 <= i < index ==> (#[trigger] self.accounts@[i]).id != id,
            decreases self.accounts@.len() - index,
        {
            if self.accounts[index].id.0 == id.0 {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn name_at(&self, name: &Name) -> (at: Option<usize>)
        ensures
            match at {
                Some(at) => at < self.accounts@.len() && self.accounts@[at as int].name@ == name@,
                None => forall|i: int|
                    0 <= i < self.accounts@.len() ==> (#[trigger] self.accounts@[i]).name@ != name@,
            },
    {
        let mut index = 0;
        while index < self.accounts.len()
            invariant
                index <= self.accounts@.len(),
                forall|i: int| 0 <= i < index ==> (#[trigger] self.accounts@[i]).name@ != name@,
            decreases self.accounts@.len() - index,
        {
            if self.accounts[index].name.as_str() == name.as_str() {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn owner_at(&self) -> (at: Option<usize>)
        ensures
            match at {
                Some(at) => at < self.accounts@.len() && self.accounts@[at as int].owner,
                None => forall|i: int|
                    0 <= i < self.accounts@.len() ==> !(#[trigger] self.accounts@[i]).owner,
            },
    {
        let mut index = 0;
        while index < self.accounts.len()
            invariant
                index <= self.accounts@.len(),
                forall|i: int| 0 <= i < index ==> !(#[trigger] self.accounts@[i]).owner,
            decreases self.accounts@.len() - index,
        {
            if self.accounts[index].owner {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn code_at(&self, code: &Code) -> (at: Option<usize>)
        ensures
            match at {
                Some(at) => at < self.invitations@.len() && self.invitations@[at as int].code@
                    == code@,
                None => forall|i: int|
                    0 <= i < self.invitations@.len() ==> (#[trigger] self.invitations@[i]).code@
                        != code@,
            },
    {
        let mut index = 0;
        while index < self.invitations.len()
            invariant
                index <= self.invitations@.len(),
                forall|i: int| 0 <= i < index ==> (#[trigger] self.invitations@[i]).code@ != code@,
            decreases self.invitations@.len() - index,
        {
            if self.invitations[index].code.as_str() == code.as_str() {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn open_owner_code_at(&self, now: Timestamp) -> (at: Option<usize>)
        ensures
            at matches Some(at) ==> at < self.invitations@.len() && self.invitations@[at as int].owner
                && is_open(self.invitations@[at as int], now.millis()),
    {
        let mut index = 0;
        while index < self.invitations.len()
            invariant
                index <= self.invitations@.len(),
            decreases self.invitations@.len() - index,
        {
            if self.invitations[index].owner && invitation_is_open(&self.invitations[index], now) {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn reserved_at(&self, name: &Name) -> (at: Option<usize>)
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

fn owner_invitation(code: &Code, now: Timestamp) -> (invitation: Invitation)
    ensures
        invitation.code@ == code@,
        invitation.owner,
        invitation.used == 0,
        invitation.uses == Some(1u32),
{
    Invitation {
        code: code.clone(),
        owner: true,
        maker: None,
        created: now,
        end: None,
        uses: Some(1),
        used: 0,
        revoked: None,
    }
}

} // verus!

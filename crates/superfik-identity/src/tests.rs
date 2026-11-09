use superfik_core::access::OrgRole;
use superfik_core::{Member, OrgId, Principal};

use crate::{Actor, CODE_LIFETIME, Directory, IdentityError, InMemoryAccounts, LOCK_SECONDS, TOKEN_LIFETIME};

const PASSWORD: &str = "correct horse";

fn directory() -> Directory<InMemoryAccounts> {
    Directory::new(OrgId::new(), InMemoryAccounts::default())
}

fn with_owner() -> (Directory<InMemoryAccounts>, Member) {
    let directory = directory();
    let (olga, code) = directory.add(Actor::Console, "Olga", OrgRole::Owner, 0).expect("owner");
    directory.first_access("olga", &code, PASSWORD, 1).expect("first access");
    (directory, olga)
}

#[test]
fn a_user_added_with_a_code_chooses_a_password_then_signs_in_with_it() {
    let directory = directory();
    let (olga, code) = directory.add(Actor::Console, " Olga ", OrgRole::Owner, 0).expect("add");
    assert!(directory.named("olga").expect("named").pending());
    assert_eq!(directory.sign_in("olga", PASSWORD, 1), Err(IdentityError::WrongCredentials), "no hint it exists");
    let token = directory.first_access("OLGA", &code.to_lowercase(), PASSWORD, 1).expect("first access");
    assert_eq!(directory.authenticate(&token, 2), Some(Principal::Member(olga)));
    assert_eq!(directory.first_access("olga", &code, PASSWORD, 2), Err(IdentityError::WrongCode), "spent");
    let again = directory.sign_in("olga", PASSWORD, 3).expect("sign in");
    assert_eq!(directory.authenticate(&again, 4), Some(Principal::Member(olga)));
    assert_eq!(directory.sign_in("olga", "wrong password", 5), Err(IdentityError::WrongCredentials));
}

#[test]
fn a_code_expires_and_a_short_password_is_refused() {
    let directory = directory();
    let (_, code) = directory.add(Actor::Console, "ana", OrgRole::Member, 0).expect("add");
    assert_eq!(directory.first_access("ana", &code, "short", 1), Err(IdentityError::WeakPassword));
    assert_eq!(directory.first_access("ana", &code, PASSWORD, CODE_LIFETIME), Err(IdentityError::WrongCode));
}

#[test]
fn a_sign_in_lasts_until_it_expires_or_ends() {
    let (directory, olga) = with_owner();
    let token = directory.sign_in("olga", PASSWORD, 10).expect("token");
    assert_eq!(directory.authenticate(&token, 10 + TOKEN_LIFETIME - 1), Some(Principal::Member(olga)));
    assert_eq!(directory.authenticate(&token, 10 + TOKEN_LIFETIME), None);
    directory.sign_out(&token).expect("out");
    assert_eq!(directory.authenticate(&token, 11), None);
    assert_eq!(directory.authenticate("made up", 11), None);
}

#[test]
fn usernames_are_unique_and_plain() {
    let (directory, olga) = with_owner();
    assert_eq!(directory.add(Actor::Member(olga), "OLGA", OrgRole::Member, 0), Err(IdentityError::NameTaken));
    assert_eq!(directory.add(Actor::Member(olga), "ana silva", OrgRole::Member, 0), Err(IdentityError::BadName));
    assert_eq!(directory.add(Actor::Member(olga), "", OrgRole::Member, 0), Err(IdentityError::BadName));
    assert!(directory.add(Actor::Member(olga), "ana.silva_2", OrgRole::Member, 0).is_ok());
}

#[test]
fn admins_manage_everyone_but_owners_and_members_manage_nobody() {
    let (directory, olga) = with_owner();
    let (ana, _) = directory.add(Actor::Member(olga), "ana", OrgRole::Admin, 0).expect("admin");
    let (bia, _) = directory.add(Actor::Member(ana), "bia", OrgRole::Member, 0).expect("member");
    assert_eq!(directory.add(Actor::Member(ana), "eve", OrgRole::Owner, 0), Err(IdentityError::Forbidden));
    assert_eq!(directory.add(Actor::Member(bia), "eve", OrgRole::Member, 0), Err(IdentityError::Forbidden));
    assert_eq!(directory.remove(Actor::Member(ana), olga.user), Err(IdentityError::Forbidden));
    assert_eq!(directory.set_role(Actor::Member(ana), bia.user, OrgRole::Guest), Ok(()));
    assert_eq!(directory.role(&bia), Some(OrgRole::Guest));
}

#[test]
fn a_new_code_ends_the_password_and_every_sign_in() {
    let (directory, olga) = with_owner();
    let (ana, code) = directory.add(Actor::Member(olga), "ana", OrgRole::Member, 0).expect("add");
    let token = directory.first_access("ana", &code, PASSWORD, 1).expect("first");
    let fresh = directory.new_code(Actor::Member(olga), ana.user, 2).expect("new code");
    assert_eq!(directory.authenticate(&token, 3), None);
    assert_eq!(directory.sign_in("ana", PASSWORD, 3), Err(IdentityError::WrongCredentials));
    assert!(directory.first_access("ana", &fresh, "another password", 4).is_ok());
}

#[test]
fn removed_users_lose_their_sign_ins_and_the_last_owner_stays() {
    let (directory, olga) = with_owner();
    let (ana, code) = directory.add(Actor::Member(olga), "ana", OrgRole::Member, 0).expect("add");
    let token = directory.first_access("ana", &code, PASSWORD, 1).expect("first");
    directory.remove(Actor::Member(olga), ana.user).expect("remove");
    assert_eq!(directory.authenticate(&token, 2), None);
    assert_eq!(directory.role(&ana), None);
    assert_eq!(directory.remove(Actor::Console, olga.user), Err(IdentityError::LastOwner));
    assert_eq!(directory.set_role(Actor::Console, olga.user, OrgRole::Admin), Err(IdentityError::LastOwner));
    assert!(directory.add(Actor::Member(olga), "ana", OrgRole::Member, 3).is_ok(), "the name is free again");
}

#[test]
fn too_many_wrong_passwords_lock_the_name_for_a_while() {
    let (directory, _) = with_owner();
    for _ in 0..crate::FAILURES_BEFORE_LOCK {
        assert_eq!(directory.sign_in("olga", "wrong password", 10), Err(IdentityError::WrongCredentials));
    }
    assert_eq!(directory.sign_in("olga", PASSWORD, 11), Err(IdentityError::TooManyAttempts));
    assert!(directory.sign_in("olga", PASSWORD, 10 + LOCK_SECONDS).is_ok());
}

#[test]
fn unknown_and_pending_names_fail_exactly_like_a_wrong_password() {
    let (directory, olga) = with_owner();
    directory.add(Actor::Member(olga), "ana", OrgRole::Member, 0).expect("add");
    for name in ["nobody", "ana", "olga"] {
        assert_eq!(directory.sign_in(name, "wrong password", 10), Err(IdentityError::WrongCredentials), "{name}");
    }
}

#[test]
fn the_tally_is_free_while_a_password_is_checked() {
    let (directory, _) = with_owner();
    let outcome = directory.throttled("olga", 10, || {
        assert!(directory.attempts.try_lock().is_ok(), "other sign-ins wait on this one");
        Err::<(), _>(IdentityError::WrongCredentials)
    });
    assert_eq!(outcome, Err(IdentityError::WrongCredentials));
    assert_eq!(directory.tracked(), 1, "and the failure is still counted");
}

#[test]
fn names_that_cannot_be_usernames_are_not_counted() {
    let (directory, _) = with_owner();
    for name in ["x".repeat(40).as_str(), "bad name!", ""] {
        assert_eq!(directory.sign_in(name, "wrong password", 10), Err(IdentityError::WrongCredentials));
    }
    assert_eq!(directory.tracked(), 0);
}

#[test]
fn old_failures_are_forgotten_and_the_tally_stays_bounded() {
    let (directory, _) = with_owner();
    let fail = |name: &str, now| directory.throttled(name, now, || Err::<(), _>(IdentityError::WrongCredentials));
    for i in 0..crate::MAX_TRACKED {
        assert_eq!(fail(&format!("n{i}"), 10), Err(IdentityError::WrongCredentials));
    }
    assert_eq!(directory.tracked(), crate::MAX_TRACKED);
    assert_eq!(fail("olga", 11), Err(IdentityError::TooManyAttempts), "full: refuse rather than forget");
    assert_eq!(fail("olga", 10 + LOCK_SECONDS), Err(IdentityError::WrongCredentials), "stale ones pruned");
    assert_eq!(directory.tracked(), 1);
}

#[test]
fn a_fresh_server_has_no_owner_until_the_command_line_adds_one() {
    let directory = directory();
    assert_eq!(directory.has_owner(), Ok(false));
    directory.add(Actor::Console, "root", OrgRole::Owner, 0).expect("owner");
    assert_eq!(directory.has_owner(), Ok(true));
}

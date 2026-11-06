//! Publishing a value for readers.
#![expect(clippy::unwrap_used, reason = "a failed test is the intended report")]

use engine_cache::{
    AccountingDomain, AllocationClass, Backing, Dependencies, DependencyError, DependencyScope,
    InstallErrorReason, Lookup, Publication, PublicationRejectionReason, StoredPayload,
};

fn backing(value: u8, domain: &AccountingDomain) -> Backing<'static, u8> {
    Backing::try_new(value, domain, 1, AllocationClass::Resident).unwrap()
}
fn read(
    publication: &Publication<'_, u8, u8>,
    id: &engine_cache::Identity<u8>,
    deps: &Dependencies,
) -> u8 {
    match publication.lookup(id, deps).unwrap() {
        Lookup::Ready(view) => *view.get(),
        _ => panic!("expected ready payload"),
    }
}

#[test]
fn attempt_replacement_and_candidates_select_one_owned_winner() {
    let domain = AccountingDomain::new();
    let deps = Dependencies::new();
    let mut publication = Publication::<u8, u8>::new();
    let id = publication.ensure(1);
    let snapshot = deps.snapshot(&[]).unwrap();
    let obsolete = publication.begin_attempt(&id, snapshot.clone()).unwrap();
    let current = publication.begin_attempt(&id, snapshot).unwrap();
    let candidate = current.clone();
    let rejected = publication
        .publish(&obsolete, &deps, Ok(backing(9, &domain)), 1)
        .unwrap_err();
    assert_eq!(
        rejected.reason(),
        PublicationRejectionReason::ObsoleteAttempt
    );
    assert_eq!(*rejected.into_parts().0.unwrap().view().get(), 9);
    assert!(matches!(
        publication
            .publish(&current, &deps, Ok(backing(2, &domain)), 1)
            .unwrap(),
        StoredPayload::Vacant
    ));
    let loser = publication
        .publish(&candidate, &deps, Ok(backing(3, &domain)), 1)
        .unwrap_err();
    assert_eq!(loser.reason(), PublicationRejectionReason::ObsoleteAttempt);
    assert_eq!(*loser.into_parts().0.unwrap().view().get(), 3);
    assert_eq!(read(&publication, &id, &deps), 2);
}

#[test]
fn authoritative_updates_clear_and_switch_have_distinct_authority_rules() {
    let domain = AccountingDomain::new();
    let deps = Dependencies::new();
    let mut publication = Publication::<u8, u8>::new();
    let id = publication.ensure(1);
    let authority = publication.authority(&id).unwrap();
    let snapshot = || deps.snapshot(&[]).unwrap();
    publication
        .authoritative(
            &id,
            Some(&authority),
            &deps,
            &snapshot(),
            Ok(backing(1, &domain)),
            1,
        )
        .unwrap();
    let retained = match publication.share(&id, &deps).unwrap() {
        Lookup::Ready(backing) => backing,
        _ => panic!("expected owner"),
    };
    let superseded_by_update = publication.begin_attempt(&id, snapshot()).unwrap();
    let displaced = publication
        .authoritative(
            &id,
            Some(&authority),
            &deps,
            &snapshot(),
            Ok(backing(8, &domain)),
            1,
        )
        .unwrap();
    match displaced {
        StoredPayload::Ready(owner) => assert_eq!(*owner.view().get(), 1),
        _ => panic!("expected displaced owner"),
    }
    assert_eq!(read(&publication, &id, &deps), 8);
    assert_eq!(*retained.view().get(), 1);
    assert_eq!(
        publication
            .publish(&superseded_by_update, &deps, Ok(backing(9, &domain)), 1)
            .unwrap_err()
            .reason(),
        PublicationRejectionReason::ObsoleteAttempt
    );
    let attempt = publication.begin_attempt(&id, snapshot()).unwrap();
    let old = publication.clear(&id).unwrap();
    drop(old);
    publication
        .authoritative(&id, None, &deps, &snapshot(), Ok(backing(2, &domain)), 1)
        .unwrap();
    assert_eq!(read(&publication, &id, &deps), 2);
    assert_eq!(*retained.view().get(), 1);
    // A clear preserves the tagged authority as well as untagged delivery.
    publication
        .authoritative(
            &id,
            Some(&authority),
            &deps,
            &snapshot(),
            Ok(backing(2, &domain)),
            1,
        )
        .unwrap();
    let rejected = publication
        .publish(&attempt, &deps, Ok(backing(3, &domain)), 1)
        .unwrap_err();
    assert_eq!(
        rejected.reason(),
        PublicationRejectionReason::ObsoleteAttempt
    );
    drop(publication.replace_authority(&id));
    let rejected = publication
        .authoritative(
            &id,
            Some(&authority),
            &deps,
            &snapshot(),
            Ok(backing(4, &domain)),
            1,
        )
        .unwrap_err();
    assert_eq!(
        rejected.reason(),
        PublicationRejectionReason::AuthorityChanged
    );
    assert!(matches!(
        publication.lookup(&id, &deps).unwrap(),
        Lookup::Vacant
    ));
    assert_eq!(*retained.view().get(), 1);
}

#[test]
fn dependency_invalidation_blocks_reuse_and_stale_attempt_without_revoking_readers() {
    let domain = AccountingDomain::new();
    let mut deps = Dependencies::new();
    let source = deps.register(DependencyScope::Source);
    let device = deps.register(DependencyScope::Device);
    let mut publication = Publication::<u8, u8>::new();
    let id = publication.ensure(1);
    let source_snapshot = deps.snapshot(std::slice::from_ref(&source)).unwrap();
    publication
        .authoritative(
            &id,
            None,
            &deps,
            &source_snapshot,
            Ok(backing(1, &domain)),
            1,
        )
        .unwrap();
    let retained = match publication.share(&id, &deps).unwrap() {
        Lookup::Ready(backing) => backing,
        _ => panic!("expected owner"),
    };
    deps.advance(&[device]).unwrap();
    assert_eq!(read(&publication, &id, &deps), 1);
    let attempt = publication
        .begin_attempt(&id, source_snapshot.clone())
        .unwrap();
    deps.advance(&[source]).unwrap();
    assert!(matches!(
        publication.lookup(&id, &deps),
        Err(DependencyError::Stale)
    ));
    assert!(matches!(
        publication.share(&id, &deps),
        Err(DependencyError::Stale)
    ));
    let rejected = publication
        .publish(&attempt, &deps, Ok(backing(2, &domain)), 1)
        .unwrap_err();
    assert_eq!(
        rejected.reason(),
        PublicationRejectionReason::Dependency(DependencyError::Stale)
    );
    // The authoritative call borrowed the original stamp; retrying the same
    // captured result cannot quietly relabel it with the new source revision.
    let rejected = publication
        .authoritative(
            &id,
            None,
            &deps,
            &source_snapshot,
            Ok(backing(3, &domain)),
            1,
        )
        .unwrap_err();
    assert_eq!(
        rejected.reason(),
        PublicationRejectionReason::Dependency(DependencyError::Stale)
    );
    assert_eq!(*rejected.into_parts().0.unwrap().view().get(), 3);
    assert_eq!(*retained.view().get(), 1);
}

#[test]
fn install_rejection_preserves_attempt_and_failure_is_a_selected_outcome() {
    let domain = AccountingDomain::new();
    let deps = Dependencies::new();
    let mut publication = Publication::<u8, u8, String>::new();
    let id = publication.ensure(1);
    let attempt = publication
        .begin_attempt(&id, deps.snapshot(&[]).unwrap())
        .unwrap();
    let staging = Backing::try_new(2, &domain, 1, AllocationClass::Retiring).unwrap();
    let rejected = publication
        .publish(&attempt, &deps, Ok(staging), 7)
        .unwrap_err();
    assert_eq!(
        rejected.reason(),
        PublicationRejectionReason::Install(InstallErrorReason::InvalidClass)
    );
    let (payload, cost) = rejected.into_parts();
    assert_eq!(cost, 7);
    assert_eq!(*payload.unwrap().view().get(), 2);
    // Retry the same ticket before any authoritative update can replace it.
    publication
        .publish(&attempt, &deps, Ok(backing(4, &domain)), 1)
        .unwrap();
    assert!(
        matches!(publication.lookup(&id, &deps).unwrap(), Lookup::Ready(view) if *view.get() == 4)
    );
    publication
        .authoritative(
            &id,
            None,
            &deps,
            &deps.snapshot(&[]).unwrap(),
            Ok(backing(5, &domain)),
            1,
        )
        .unwrap();
    let other = publication.ensure(2);
    publication
        .authoritative(
            &other,
            None,
            &deps,
            &deps.snapshot(&[]).unwrap(),
            Ok(backing(6, &domain)),
            u64::MAX - 1,
        )
        .unwrap();
    let attempt = publication
        .begin_attempt(&id, deps.snapshot(&[]).unwrap())
        .unwrap();
    let rejected = publication
        .publish(&attempt, &deps, Ok(backing(8, &domain)), 2)
        .unwrap_err();
    assert_eq!(
        rejected.reason(),
        PublicationRejectionReason::Install(InstallErrorReason::PolicyOverflow)
    );
    let (payload, cost) = rejected.into_parts();
    assert_eq!(cost, 2);
    assert_eq!(*payload.unwrap().view().get(), 8);
    assert_eq!(publication.policy_bytes(), u64::MAX);
    assert!(
        matches!(publication.lookup(&id, &deps).unwrap(), Lookup::Ready(view) if *view.get() == 5)
    );
    publication
        .publish(&attempt, &deps, Err("accepted failure".to_owned()), 0)
        .unwrap();
    assert!(
        matches!(publication.lookup(&id, &deps).unwrap(), Lookup::Failed(error) if error == "accepted failure")
    );
}

#[test]
fn removed_and_recycled_membership_never_inherits_authority() {
    let domain = AccountingDomain::new();
    let deps = Dependencies::new();
    let mut publication = Publication::<u8, u8>::new();
    let old = publication.ensure(1);
    let attempt = publication
        .begin_attempt(&old, deps.snapshot(&[]).unwrap())
        .unwrap();
    let authority = publication.authority(&old).unwrap();
    drop(publication.remove(&1));
    let new = publication.ensure(1);
    assert_ne!(old, new);
    assert_eq!(
        publication
            .publish(&attempt, &deps, Ok(backing(1, &domain)), 1)
            .unwrap_err()
            .reason(),
        PublicationRejectionReason::Missing
    );
    assert_eq!(
        publication
            .authoritative(
                &new,
                Some(&authority),
                &deps,
                &deps.snapshot(&[]).unwrap(),
                Ok(backing(2, &domain)),
                1
            )
            .unwrap_err()
            .reason(),
        PublicationRejectionReason::AuthorityChanged
    );
    assert!(matches!(
        publication.lookup(&new, &deps).unwrap(),
        Lookup::Vacant
    ));
}

#[test]
fn foreign_snapshot_update_returns_payload_without_revoking_pending_attempt() {
    let domain = AccountingDomain::new();
    let deps = Dependencies::new();
    let foreign = Dependencies::new();
    let mut publication = Publication::<u8, u8>::new();
    let id = publication.ensure(1);
    let attempt = publication
        .begin_attempt(&id, deps.snapshot(&[]).unwrap())
        .unwrap();
    let rejected = publication
        .publish(&attempt, &foreign, Ok(backing(6, &domain)), 2)
        .unwrap_err();
    assert_eq!(
        rejected.reason(),
        PublicationRejectionReason::Dependency(DependencyError::ForeignRegistry)
    );
    let (payload, cost) = rejected.into_parts();
    assert_eq!(cost, 2);
    assert_eq!(*payload.unwrap().view().get(), 6);
    let rejected = publication
        .authoritative(
            &id,
            None,
            &deps,
            &foreign.snapshot(&[]).unwrap(),
            Ok(backing(7, &domain)),
            3,
        )
        .unwrap_err();
    assert_eq!(
        rejected.reason(),
        PublicationRejectionReason::Dependency(DependencyError::ForeignRegistry)
    );
    let (payload, cost) = rejected.into_parts();
    assert_eq!(cost, 3);
    assert_eq!(*payload.unwrap().view().get(), 7);
    publication
        .publish(&attempt, &deps, Ok(backing(2, &domain)), 1)
        .unwrap();
    assert_eq!(read(&publication, &id, &deps), 2);
}

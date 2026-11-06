//! Publishing a new version of a value beside the old.
#![expect(clippy::unwrap_used, reason = "an example keeps to what it shows")]
#![expect(clippy::print_stdout, reason = "an example reports to its console")]

use engine_cache::{
    AccountingDomain, AllocationClass, Backing, Dependencies, DependencyError, DependencyScope,
    Lookup, Production, ProductionClaim, ProviderDecision, Publication,
};

fn main() {
    let accounting = AccountingDomain::new();
    let mut dependencies = Dependencies::new();
    let source = dependencies.register(DependencyScope::Source);
    let mut publication = Publication::<_, Vec<u8>, &'static str>::new();
    let identity = publication.ensure("document");
    let attempt = publication
        .begin_attempt(
            &identity,
            dependencies
                .snapshot(std::slice::from_ref(&source))
                .unwrap(),
        )
        .unwrap();
    let (production, submission) = Production::prepare(vec![1, 2, 3]);
    submission.submit(|mut producer| {
        let bytes = producer
            .inputs()
            .unwrap()
            .iter()
            .map(|byte| byte * 2)
            .collect::<Vec<_>>();
        let backing = Backing::try_new(bytes, &accounting, 3, AllocationClass::Resident).unwrap();
        producer.complete(Ok(backing)).unwrap();
        ProviderDecision::Accepted
    });
    let ProductionClaim::Ready(outcome) = production.claim() else {
        panic!("inline provider has settled all access");
    };
    publication
        .publish(&attempt, &dependencies, outcome, 3)
        .unwrap();
    let Lookup::Ready(reader) = publication.share(&identity, &dependencies).unwrap() else {
        panic!("published resource");
    };
    dependencies.advance(std::slice::from_ref(&source)).unwrap();
    assert!(matches!(
        publication.lookup(&identity, &dependencies),
        Err(DependencyError::Stale)
    ));
    assert_eq!(reader.view().get(), &[2, 4, 6]);
    // Old readers retain their allocation; new readers need a current publication.
    drop(publication.clear(&identity));
    assert_eq!(accounting.snapshot().resident_bytes, 3);
    drop(reader);
    assert_eq!(accounting.snapshot().total_declared_bytes, 0);
    println!("Source invalidation blocks new reuse while old readers retain backing.");
}

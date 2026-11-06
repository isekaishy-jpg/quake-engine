//! A cache from its start to its end.

use engine_cache::{
    AccountingDomain, Admission, AdmissionError, AllocationClass, Backing, Cache, CountLimit,
    ProductionClaim, ProductionMap, ProductionStart, ProviderDecision, StartRejectionReason,
    SubmissionOutcome,
};

#[test]
fn root_closure_is_permanent_and_settles_only_its_reservations() {
    let admission = Admission::new(CountLimit::new(1));
    assert!(!admission.snapshot().is_drained());
    let permit = admission.try_acquire().unwrap();
    let clone = admission.clone();
    let accounting = AccountingDomain::new();
    let escaped = Backing::try_new(7, &accounting, 16, AllocationClass::Resident).unwrap();
    clone.close();
    admission.close();
    admission.set_limit(CountLimit::new(10));
    assert!(admission.snapshot().closed);
    assert_eq!(clone.snapshot().active, 1);
    assert!(!clone.snapshot().is_drained());
    assert_eq!(admission.try_acquire().unwrap_err(), AdmissionError::Closed);
    drop(permit);
    assert_eq!(admission.snapshot().active, 0);
    assert!(admission.snapshot().is_drained());
    assert_eq!(clone.try_acquire().unwrap_err(), AdmissionError::Closed);
    assert_eq!(*escaped.view().get(), 7);
    assert_eq!(accounting.snapshot().resident_bytes, 16);
    drop(escaped);
    assert_eq!(accounting.snapshot().resident_bytes, 0);
}

#[test]
fn closed_roots_preserve_inputs_and_allow_accepted_join_and_discovery() {
    let admission = Admission::new(CountLimit::new(2));
    let mut cache = Cache::<u8, u8>::new();
    let identity = cache.ensure(1);
    let other = cache.ensure(2);
    let mut map = ProductionMap::<u8, u8, ()>::new(admission.clone());
    let _interest = map.interest(identity.clone(), 0, 0);
    let _other_interest = map.interest(other.clone(), 0, 0);
    let attempt = match map
        .begin_shared(&identity, String::from("accepted"))
        .unwrap()
    {
        ProductionStart::Started(attempt) => attempt,
        ProductionStart::Joined { .. } => panic!(),
    };
    admission.close();
    match map.begin_shared(&identity, "prepared join").unwrap() {
        ProductionStart::Joined { unused_input, .. } => assert_eq!(unused_input, "prepared join"),
        ProductionStart::Started(_) => panic!("closure cannot create another root"),
    }
    // Preparation already acquired its permit; closure does not revoke it.
    let mut producer = None;
    assert!(matches!(
        attempt.submission.submit(|root| {
            producer = Some(root);
            ProviderDecision::Accepted
        }),
        SubmissionOutcome::Accepted
    ));
    let input = Box::new(String::from("rejected"));
    let address = std::ptr::from_ref(&*input);
    let rejected = match map.begin_shared(&other, input) {
        Err(rejected) => rejected,
        Ok(_) => panic!(),
    };
    assert_eq!(rejected.reason(), StartRejectionReason::Closed);
    let returned = rejected.into_inputs();
    assert_eq!(std::ptr::from_ref(&*returned), address);
    let rejected = match map.begin_candidate(&identity, "candidate") {
        Err(rejected) => rejected,
        Ok(_) => panic!(),
    };
    assert_eq!(rejected.reason(), StartRejectionReason::Closed);
    assert_eq!(rejected.into_inputs(), "candidate");
    match map.begin_shared(&identity, "join").unwrap() {
        ProductionStart::Joined { unused_input, .. } => assert_eq!(unused_input, "join"),
        ProductionStart::Started(_) => panic!(),
    }
    let mut root = producer.take().unwrap();
    let child = root.child("discovered after close").unwrap();
    root.complete(Ok(9)).unwrap();
    drop(root);
    let nested = child.child("nested discovery").unwrap();
    drop(child);
    assert_eq!(admission.snapshot().active, 1);
    assert!(matches!(
        attempt.production.claim(),
        ProductionClaim::Pending
    ));
    drop(nested);
    assert_eq!(admission.snapshot().active, 0);
    assert!(matches!(
        attempt.production.claim(),
        ProductionClaim::Ready(Ok(9))
    ));
}

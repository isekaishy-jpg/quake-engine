#![expect(clippy::print_stdout, reason = "a test reports to its console")]

use super::*;

fn fixture() -> (HostId, RawTime, Services) {
    let host = HostId::allocate().unwrap();
    (
        host,
        RawTime {
            host,
            elapsed: Duration::from_millis(10),
        },
        Services::new(),
    )
}
fn register(
    services: &mut Services,
    host: HostId,
    raw: RawTime,
    points: ServicePoints,
) -> ServiceId {
    services
        .register(
            host,
            ServiceSpec {
                points,
                budget: ServiceBudget { records: 1 },
                fallback_interval: Duration::from_millis(10),
            },
            raw,
            crate::backend::wake::NativeWake::new(),
        )
        .unwrap()
}
fn oracle(services: &Services, point: ServicePoint, raw: RawTime) {
    let pending = services
        .records
        .iter()
        .any(|(_, record)| Services::due(record, point, raw));
    let deadline = services
        .records
        .iter()
        .filter(|(_, record)| record.alive && !record.active && record.spec.points.contains(point))
        .map(|(_, record)| {
            if record.wake.pending() {
                RawDeadline(raw)
            } else {
                record.next
            }
        })
        .min_by_key(|deadline| deadline.time().elapsed);
    let active = services
        .records
        .iter()
        .filter(|(_, record)| record.active)
        .count();
    let retirement = services
        .records
        .iter()
        .filter(|(_, record)| record.alive && record.spec.points.contains(ServicePoint::Retirement))
        .count();
    assert_eq!(services.pending(point, raw), pending);
    assert_eq!(services.deadline(point, raw), deadline);
    assert_eq!(services.counts(), (active, retirement));
}

#[test]
fn occupied_queries_match_historical_scan_for_mixed_reports_and_retained_active_records() {
    let (host, raw, mut services) = fixture();
    let mut ids = Vec::new();
    for index in 0..48 {
        let points = if index % 2 == 0 {
            ServicePoints::ALL
        } else {
            ServicePoints::ORDINARY
        };
        ids.push(register(&mut services, host, raw, points));
    }
    for index in 0..48 {
        let request = services.next(host, ServicePoint::Maintenance, raw).unwrap();
        let report = match index % 4 {
            0 => ServiceReport::Quiescent,
            1 => ServiceReport::Continue,
            2 => ServiceReport::WaitUntil(raw.checked_add(Duration::from_millis(30)).unwrap()),
            _ => ServiceReport::AwaitWake {
                source: request.id,
                fallback_deadline: raw.checked_add(Duration::from_millis(5)).unwrap(),
            },
        };
        services.release(host, request, Some(report)).unwrap();
    }
    for id in ids.iter().step_by(3) {
        services.get(host, *id).unwrap().wake.signal().unwrap();
    }
    let active = services.next(host, ServicePoint::Maintenance, raw).unwrap();
    let late_wake = services.get(host, active.id).unwrap().wake.clone();
    assert!(services.remove(host, active.id).unwrap().active);
    assert_eq!(late_wake.signal(), Err(Error::AdmissionClosed));
    for point in [
        ServicePoint::PreUpdate,
        ServicePoint::Maintenance,
        ServicePoint::Explicit,
        ServicePoint::Retirement,
    ] {
        oracle(&services, point, raw);
        oracle(
            &services,
            point,
            raw.checked_add(Duration::from_secs(1)).unwrap().time(),
        );
    }
    services.close_all();
    assert_eq!(services.counts(), (1, 0));
    oracle(&services, ServicePoint::Retirement, raw);
    services.release(host, active, None).unwrap();
    assert_eq!(services.counts(), (0, 0));
}

#[test]
fn query_work_tracks_occupied_records_after_a_large_historical_peak() {
    let (host, raw, mut services) = fixture();
    let ids: Vec<_> = (0..4096)
        .map(|_| register(&mut services, host, raw, ServicePoints::ALL))
        .collect();
    for id in &ids[..4093] {
        services.remove(host, *id).unwrap();
    }
    for id in &ids[4093..] {
        let record = services.records.get_mut(id.key).unwrap();
        record.wake.consume();
        record.next = raw.checked_add(Duration::from_secs(1)).unwrap();
    }
    let (_, allocations) = crate::allocation_probe::measure(|| {
        services.counts();
        services.deadline(ServicePoint::Maintenance, raw);
        services.pending(ServicePoint::PreUpdate, raw);
    });
    assert_eq!(allocations.allocations + allocations.reallocations, 0);
    services.query_visits.set(0);
    assert_eq!(services.counts(), (0, 3));
    assert_eq!(services.query_visits.get(), 3);
    services.query_visits.set(0);
    services.deadline(ServicePoint::Maintenance, raw);
    assert_eq!(services.query_visits.get(), 3);
    services.query_visits.set(0);
    assert!(!services.pending(ServicePoint::PreUpdate, raw));
    assert_eq!(services.query_visits.get(), 3);
    for id in &ids[4093..] {
        services.remove(host, *id).unwrap();
    }
    services.query_visits.set(0);
    assert_eq!(services.counts(), (0, 0));
    assert_eq!(services.deadline(ServicePoint::Maintenance, raw), None);
    assert!(!services.pending(ServicePoint::Maintenance, raw));
    assert_eq!(services.query_visits.get(), 0);
    println!(
        "service-query: historical=4096 occupied=3 counts/deadline/pending visits=3 each; empty visits=0"
    );
}

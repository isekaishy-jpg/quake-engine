use super::*;

#[expect(
    clippy::cast_possible_truncation,
    reason = "a structure is far smaller than 4 GB"
)]
fn core(group: u16, mask: u64, efficiency: u8) -> Vec<u8> {
    let header = offset_of!(SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX, Anonymous);
    let size = header + size_of::<PROCESSOR_RELATIONSHIP>();
    let mut data = vec![0; size];
    data[..4].copy_from_slice(&RelationProcessorCore.0.to_le_bytes());
    data[4..8].copy_from_slice(&(size as u32).to_le_bytes());
    let count = header + offset_of!(PROCESSOR_RELATIONSHIP, GroupCount);
    data[count..count + 2].copy_from_slice(&1u16.to_le_bytes());
    data[header + offset_of!(PROCESSOR_RELATIONSHIP, EfficiencyClass)] = efficiency;
    let affinity = header + offset_of!(PROCESSOR_RELATIONSHIP, GroupMask);
    data[affinity..affinity + 8].copy_from_slice(&mask.to_le_bytes());
    let group_offset = affinity + offset_of!(GROUP_AFFINITY, Group);
    data[group_offset..group_offset + 2].copy_from_slice(&group.to_le_bytes());
    data
}

#[test]
fn groups_and_heterogeneous_cores_remain_distinct() {
    let mut data = core(0, 3, 8);
    data.extend(core(1, 3, 0));
    let topology = parse(&data).unwrap();
    assert_eq!(topology.physical_cores(), 2);
    assert_eq!(topology.logical_processors(), 4);
    assert_eq!(topology.cores()[0].efficiency_class(), 8);
    assert_eq!(topology.cores()[1].affinities()[0].group, 1);
}

#[test]
fn malformed_native_records_do_not_invent_counts() {
    assert!(parse(&[]).is_err());
    let valid = core(0, 3, 0);
    for length in 0..valid.len() {
        assert!(parse(&valid[..length]).is_err());
    }
    let mut duplicated = valid.clone();
    duplicated.extend(valid.clone());
    assert!(parse(&duplicated).is_err());
    assert!(parse(&core(0, 0, 0)).is_err());
    let mut zero_size = valid;
    zero_size[4..8].fill(0);
    assert!(parse(&zero_size).is_err());
}

#[test]
fn current_windows_topology_is_observable() {
    let topology = query().unwrap();
    assert!(topology.physical_cores() > 0);
    assert!(topology.logical_processors() >= topology.physical_cores());
}

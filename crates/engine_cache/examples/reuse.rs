//! Handing a finished value back to be used again.
#![expect(clippy::unwrap_used, reason = "an example keeps to what it shows")]
#![expect(
    clippy::cast_possible_truncation,
    reason = "a test's numbers are small"
)]

use engine_cache::{
    AccountingDomain, AllocationClass, Backing, Pool, RangeStore, SourceBuffers, SourceSnapshot,
};

fn main() {
    let allocations = AccountingDomain::new();
    let source_bytes = b"captured source bytes".to_vec();
    let capacity = source_bytes.capacity() as u64;
    let source = SourceSnapshot::new(
        Backing::try_new(
            source_bytes,
            &allocations,
            capacity,
            AllocationClass::Resident,
        )
        .unwrap(),
    );
    let mut ranges = RangeStore::reference(&allocations);
    ranges
        .insert(&source, 0, &source.context().get()[..8])
        .unwrap();

    let scratch = Pool::new();
    let buffer = Vec::<u8>::with_capacity(64);
    let charge = allocations
        .charge(buffer.capacity() as u64, AllocationClass::Temporary)
        .unwrap();
    scratch.insert(1, buffer, charge).unwrap();
    let mut lease = scratch.checkout(1, capacity).unwrap().unwrap();
    lease.get_mut().resize(source.context().get().len(), 0);
    let read = ranges.copy_prefix(&source, 0, lease.get_mut()).unwrap();
    // A real provider would read the remainder through this same captured context.
    let start = read.remaining_offset() as usize;
    lease.get_mut()[read.copied()..]
        .copy_from_slice(&read.source().context().get()[start..start + read.remaining_length()]);
    assert_eq!(lease.get(), source.context().get());
    lease.reset_and_return(Vec::clear);

    let mut whole = SourceBuffers::new(0);
    let bytes = source.context().get().clone();
    let length = bytes.len() as u64;
    let charge = allocations
        .charge(bytes.capacity() as u64, AllocationClass::Resident)
        .unwrap();
    whole
        .insert_read(7, Backing::from_charged(bytes, charge), length)
        .unwrap();
    let taken = whole.take(7).unwrap();
    whole.close();
    assert_eq!(taken.view().get(), source.context().get());

    drop(taken);
    drop(read);
    ranges.close();
    scratch.close();
    drop(source);
    assert_eq!(allocations.snapshot().total_declared_bytes, 0);
}

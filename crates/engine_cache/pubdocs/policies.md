# Family policies and accounting

[Guide index](README.md)

Budgets follow Forever's family boundaries. Select reference settings explicitly
for the corresponding workload; `engine_cache` installs no global memory cap and does not
derive worker counts from resource allowances. The [research summaries](evidence.md)
explain the native-to-Rust boundary.

Component production/payload settings are not universal BLP texture-cache
defaults. Applying those settings to another workload is an explicit adaptation.
Keep a comparison at the old settings separate from a run adopting a different
policy; see [integration failure points](integration-pitfalls.md).

## Reference settings and release points

| Family | Recovered policy | `engine_cache` use and limit |
| --- | --- | --- |
| Component production | `componentTexLoadLimit` default 16; existing active entries consume the allowance. | `CACHE_REFERENCE_PRODUCTION_LIMIT` with a production `Admission`. Sixteen total active productions, not sixteen additional starts per frame. The `engine_cache` permit ends after submission settlement and final access. |
| Component retained payload | `componentTexCacheSize` default 32, converted to 32 MiB. Collection targets usage strictly below the threshold. | `CACHE_REFERENCE_PAYLOAD_RETENTION` is `AtOrAbove(32 MiB)`. Bounded `maintain` releases eligible unpinned payloads; identity can survive. Pins or the visit budget can leave pressure outstanding. |
| Provider operations | Reviewed Forever constructor starts with eight active slots; excess accepted work queues separately. | `CACHE_REFERENCE_PROVIDER_SLOTS` in a separate admission domain. The host acquires/releases slots at the provider's operation boundary. Queued ownership and descendants can outlive a slot. `engine_cache` admission itself does not supply that provider queue. |
| Copied source ranges | Sixteen records, with retained capacity separate from valid length; no recovered aggregate byte cap or ordinary shrink rule. | `CACHE_REFERENCE_RANGE_ENTRIES` is an opt-in occupancy limit for `RangeStore`. Account for actual declared capacity separately. Do not describe this as a generic byte-bounded LRU. |
| Temporary texture source buffers | A supplied nonzero soft limit is tested after insertion; zero disables the comparison. A final read can cross the limit. | `SourceBuffers` preserves this family behavior. The caller supplies the limit; no guessed OS memory-query formula is built in. Taking a buffer does not restart a stopped build scope. |
| Nonstreaming audio | Selected cache-charge removal precedes later retirement/free. | Use a family policy counter separately from backing charges. No numeric audio default is established here. |
| Released models | Age-based or forced eligible collection; captured clock values do not establish one universal duration. | Domain supplies clock/age policy if required; `engine_cache`'s implemented cache maintenance supplies eligibility and bounded visits, not an automatic native age collector. |
| Query scratch | Ordinary return retains compatible capacity; free-list drain is separate from checked-out lifetime. | Explicit `Pool` reset/return, trim and close. No universal trim threshold or query byte budget is inferred. |

The constants and exact comparisons are in [policy.rs](../src/policy.rs).
`CountLimit::allows` and `ByteLimit::reached` only compare values; they do not
reserve anything. Use `Admission` when concurrent ownership must consume a
count. A zero count limit blocks new admission, while byte-limit zero semantics
depend on the selected enum variant. Only the source-buffer family specifically
interprets its configured zero soft limit as disabled.

## Count what the policy actually counts

Source-buffer accounting intentionally demonstrates why policy and physical
ownership differ. Each successful read contributes its read size before duplicate
insertion is decided. A duplicate returns the incoming owned buffer and keeps
the first stored value, but its read-size contribution remains in the policy
counter. Taking the retained buffer subtracts that entry's read size; closing the
scope resets the policy counter. The surviving allocation owners still determine
physical lifetime. See [source-buffer tests](../tests/source_buffers.rs).

Similarly, component/audio eviction can remove a retained-family policy cost
while deferred cleanup still owns the bytes. A policy value of zero is not a
receipt that memory was freed. Overlapping old readers and replacement candidates
must all retain their allocation charges.

`AccountingDomain` records caller-declared allocation bytes in temporary,
resident, retiring and idle-capacity classes. It does not inspect the allocator
or report process memory. Declare retained capacity, including independently
owned nested allocations, rather than only a vector's current valid length.
Cloning one backing adds owners without duplicating its allocation charge.

## Transfer between execution and storage

Choose who accounts for a byte allocation during production and after delivery.
Two supported compositions are:

1. Carry an `engine_cache` allocation charge with owned bytes through `engine_worker`. The owner converts
   it to the appropriate class and binds it with `acquire_charged` or
   `Backing::from_charged`. The
   [`engine_worker` shutdown test](../examples/engine_worker/tests/shutdown.rs) uses this path.
2. Retain an `engine_worker` byte lease with an `Retained` result, acquire the `engine_cache` destination
   charge, then release the `engine_worker` lease while moving the same allocation. If the `engine_cache`
   acquisition fails, keep the original `engine_worker` result, lease and publication ticket
   for explicit retry or settlement. The
   [`engine_worker` pipeline example](../examples/engine_worker/src/lib.rs) exercises this path.

The second path briefly overlaps declarations in two independent accounting
domains. Never sum that overlap as two physical allocations. An `engine_cache`
`AllocationCharge::transfer` moves between `engine_cache` domains; it is not an automatic
`engine_worker`-to-`engine_cache` conversion. If both libraries enforce separate admission policies on the
same phase, document that policy overlap rather than silently double-counting
physical bytes.

## Service budgets are separate

`engine_cache` cache maintenance limits examined slots; cleanup `drain_budget` limits selected
records. These bounds preserve unprocessed work and expose remaining obligations.
They do not bound the wall time of an arbitrary destructor. An empty cleanup
queue does not prove there are no live owners that will enqueue later.

`engine_worker` record, edge, runnable, handoff and optional byte capacities have different
units again. Configure them for the host workload; the small capacities in the
integration example intentionally create pressure and are not Forever defaults.
Only `engine_worker`'s explicitly configured hard byte ceiling acts as its aggregate byte
ceiling. `engine_cache` family policies do not become that ceiling. See
[`engine_worker` resource capacities](https://github.com/isekaishy-jpg/engine_worker/blob/master/pubdocs/resources.md#bound-metadata-execution-pressure-and-bytes-separately).

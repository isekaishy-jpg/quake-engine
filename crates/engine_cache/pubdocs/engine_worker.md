# Direct `engine_worker` integration

[Guide index](README.md)

`engine_worker` is the base execution integration for `engine_cache`'s Stock/Forever workload
mapping. Use both public APIs directly. `engine_cache` has no `engine_worker` dependency, adapter trait or
second scheduler. The application's coordinator owns resource policy and handoff
state. The [runnable integration package](../examples/engine_worker/README.md) supplies
the actual API calls; this guide explains why the pieces belong together.

Read [integration failure points](integration-pitfalls.md) when composing an
application provider or qualifying a benchmark. Neither library implements
platform asynchronous asset I/O; external readiness must be connected to the
provider's real completion and physical-access contract.

## One shared production, one publication round

Serialize these decisions in the resource's owning coordinator. Preserve pending
state across calls to providers/`engine_worker` rather than holding a host lock across user code.

1. Determine semantic identity, representation and current dependencies. First
   share a valid ready result through `Publication::share`. A warm hit needs no
   production, new ticket or worker job. Domain-required owner delivery can still
   be needed to apply that already available result.
2. On a miss, ensure membership and attach independent consumer interest through
   `ProductionMap::interest`. Lookup alone creates no demand; `begin_shared`
   rejects when its association has no live interest. Retain the returned handle
   for that consumer's interest lifetime.
3. Call `begin_shared` with owned input. On `Started`, capture dependencies and
   create one `Publication::begin_attempt` ticket before invoking the provider.
   Pair it with this production through output, conversion and any retry.
4. On `Joined`, keep the existing production and original ticket; return or release
   unused input. Calling `begin_attempt` would revoke the running work's ticket.
5. Submit owned producer state. A provider can complete inline, so ownership,
   association and publication authority must already be established.
6. Claim once, after acceptance and every `engine_cache` producer/access guard's final use.
   One coordinator publishes; joined observers do not each get a private claim.
   Consumers share the installed backing.
7. Keep claim, conversion and publication serialized against new requests. If
   conversion/publication remains pending for retry, retain explicit owner state:
   after claim the map alone no longer prevents a new production.

`begin_shared` compares identity, not input bytes or dependency revisions. On
source/recipe change, settle old work or remove its association and establish
the new group's interests before starting replacement work. Removal neither
cancels escaped producers nor transfers old demand handles. Keep old outcomes
with old tickets; never recapture newer dependencies to make old bytes current.

`begin_candidate` is a separate explicit choice and does not replace the shared
association. Clone a publication ticket only when the domain permits independent
candidates to compete for one round. See [composition evidence](evidence.md#composition)
and [publication integration tests](../tests/publication_integration.rs).

## Admission and owned rejection

`engine_cache` family production allowance, provider slots, `engine_worker` scheduling capacity and retained
memory have different units and release points. A permit for one grants none of
the others. `engine_worker` admission errors return the uninvoked operation and relevant
options. Keep that exact closure with its `engine_cache` root/input owners for explicit retry
after progress, or settle it intentionally. Dropping an untouched retained
producer can settle abandonment; it must not strand inputs or reservations.
`TooLarge` requires a changed request/configuration or domain failure decision.

In the example, `ProviderDecision::Accepted` means the **host** accepts ownership
of the retry after `engine_worker` returns `Full`; `engine_worker` has not admitted it. A direct `engine_cache` provider
rejection must return the same root producer. A root already moved into a retained
closure cannot also be returned to `engine_cache`. This is ordinary owned-state composition,
without another adapter abstraction.

Reserve successor/delivery capacity or keep recoverable admission state before
relinquishing the current stage. Starting I/O and assuming its CPU successor must
fit can strand required work. `Runtime::external` represents provider readiness
without occupying a CPU worker: admit it before starting the provider. Physical
access and cancellation remain the provider's responsibility. Blocking I/O does
not become caller-helpable because it loads an asset.

See [provider admission evidence](evidence.md#admission-and-provider-completion),
[audio rejection evidence](evidence.md#audio-and-consumed-input) and
[`engine_worker` capacities](https://github.com/isekaishy-jpg/engine_worker/blob/master/pubdocs/resources.md).

## Demand and discovery

`engine_cache` uses larger urgency values for more urgent work; `engine_worker` resource ranks use smaller
values. Supply an explicit mapping. Select Low/Mid/High CPU execution class
separately from resource rank and provider ordering. Native distance/priority
fields are not modern API values.

Serialize `engine_cache` update, current-snapshot validation and forwarding to `Demand` or
the provider. `is_current` alone cannot prevent an older forward racing a newer
update. The example maps urgency at least 10 to rank 0 and other urgency to rank
10 as a demonstration, not a deployment policy. Tests observe actual pending
execution order after raising, lowering and detaching demand.

Drive `engine_worker` `service_demand` with a bounded budget. Rank changes do not preempt running
jobs; group completion demand does not automatically apply to every member.
Provider demand callbacks can overlap or arrive late: reject obsolete versions
and check provider lifetime. Lack of demand does not prove physical cancellation.

| Capability | Covers | Does not establish |
| --- | --- | --- |
| `engine_cache` producer/access guard | Input ownership and accepted access, including nested access after the root ends. | `engine_worker` admission authority or current domain validity. |
| `engine_worker` discovery permit | Accounted descendant admission after work-set root closure. | Source ownership or admission after work-set cancellation. |
| `engine_worker` work set | Owned producers, consumer deliveries and live discovery permits. | Lifetime of every retained output or external device user. |
| `engine_cache` interest / `engine_worker` demand | A consumer's interest and pending urgency. | Producer cancellation authority or physical final use. |

Acquire discovery permits before sealing the work set. Carry them with actual
discoverers; release them when discovery ends. `Completion::demand_in` and
`Owner::on_ready_in` attach consumer-scoped demand/delivery; their `*_from`
variants use existing discovery authority. A consumer can leave without canceling
a shared producer. Putting that producer in the consumer's canceled work set does
give the set producer cancellation authority, so choose the owner deliberately.

This serves Stock's repeated provider/owner loading waves and Forever's separate
demand handoff. See [loading](evidence.md#loading-and-discovery) and
[demand](evidence.md#demand) evidence.

## Output, publication and final use

`engine_worker` root CPU completion can precede `engine_cache` claimability while a child still accesses
input. Deliver against the relevant final-access completion, or retain an
owner-side pending claim for later service. A root group does not automatically
include independent child groups or external users.

An `engine_worker` owner callback can claim output, transfer accounting, bind `engine_cache` backing and
call `Publication::publish` in its selected phase. Rejection returns owned state.
Conversion failure must retain the bytes, original charge and original ticket.
Replacement returns displaced state; escaped readers remain valid. The
[accounting guide](policies.md#transfer-between-execution-and-storage) describes
`engine_cache`-through-`engine_worker` and `engine_worker`-to-`engine_cache` charge composition.

`CleanupContext` is scoped and neither `Send` nor `Sync`; `engine_worker` owned jobs require
`'static` captures. Move ordinary owned bytes/charges through a worker and create
scoped cleanup backing on the owner. The example keeps the context in owner state,
not in a worker closure.

| Observation | Established | Still to check |
| --- | --- | --- |
| `engine_worker` job succeeded | Execution completed under that job's contract. | Typed application outcome, `engine_cache` final access and eligible publication. |
| `engine_cache` completion recorded | An outcome was supplied. | Acceptance and every access guard ending before claim. |
| `engine_worker` delivery `Published` | Owner delivery ran successfully. | `engine_cache` acceptance; the callback may instead retain a rejected/pending candidate. |
| `engine_cache` payload published | Current authority/dependencies accepted it. | Further domain readiness, upload consumption and device final use. |
| Provider slot freed / interest detached | That slot or consumer relationship ended. | Descendant access, retained results and physical release. |

An ordinary `engine_worker` job returning `Result::Err` can still be a successfully executed
job; use the appropriate fallible API or inspect the typed result. Renderer/audio
integration must retain owners until actual final use. Controlled reader tests
check ownership, not any particular backend's physical completion detection.

## Progress and shutdown

The host drives provider service, `engine_worker` owner pumping, demand propagation and `engine_cache`
maintenance/cleanup. A passive CPU wait runs none of those services. `engine_worker` rejects
passive waits from CPU execution or live-owner contexts, including on an owner
thread outside a callback. Targeted helping executes eligible jobs from the
selected group; it does not pump owner/provider work.

Use bounded host service passes followed by the event loop or a verified wait
arrangement. Recheck actual state after notifications. An empty queue or pump
does not prove parking cannot miss a needed wakeup. The examples' finite gated
loops are test arrangements, not production event loops.

For teardown, stop new domain requests and close appropriate `engine_cache` admissions.
`Admission::close` prevents new permits from that domain. Preacquired permits
and accepted child accesses remain valid; joining existing work can still succeed.
Begin `engine_worker` shutdown, retain owner/provider service and settle accepted obligations.
Keep delivery alive while it owns unpublished output. A failed provider stop/wait
leaves ownership and a later settlement obligation, not permission to free input.

After execution/delivery settlement, release table/pool ownership as domain policy
allows and drain cleanup in its legal context. Escaped output can outlive `engine_worker` join.
`engine_cache` admission `is_drained` covers only that closed count domain's live permits; it
excludes unclaimed outputs, escaped backing, deferred cleanup and uncovered
external users. Pool close retires idle items and later returns; checked-out
leases remain owned. None is a whole-application drain receipt.

See [retirement evidence](evidence.md#retention-and-external-release),
[`engine_cache` lifecycle](README.md#closure-maintenance-and-observation) and
[`engine_worker` progress/shutdown](https://github.com/isekaishy-jpg/engine_worker/blob/master/pubdocs/lifecycle.md).

## Evidence and executable checks

| Evidence-backed requirement | Executable check | Check boundary |
| --- | --- | --- |
| Detached interest preserves input; only current results apply. | [Publication integration](../tests/publication_integration.rs): joined ticket, supersession, old readers and losing candidates. | Generic byte products, not native component code. |
| Queue rejection retains accepted responsibility. | [`engine_worker` composition](../examples/engine_worker/tests/composition.rs): genuine runnable saturation, retry and dropped untouched closure. | Instructional capacities, no fairness/performance claim. |
| Current demand reaches pending ordering. | [`engine_worker` composition](../examples/engine_worker/tests/composition.rs): raise/lower/detach changes execution order. | Demonstration rank mapping, not native numeric equivalence. |
| Descendants/access outlive roots. | [`engine_worker` pipeline](../examples/engine_worker/src/lib.rs): sealed work set, retained discovery and child guard blocking claim. | Owned CPU continuation, not a device retirement receipt. |
| Failed output transfer preserves ownership/freshness. | [`engine_worker` composition](../examples/engine_worker/tests/composition.rs): real output, `engine_worker` lease and original ticket retained, then retried. | Synthetic `u64::MAX` accounting sentinel forces arithmetic overflow; it is not allocated memory. |
| Completion does not free externally used bytes. | [External lifetime](../tests/external_lifetime.rs): controlled reader retains actual backing. | Backend must supply its own real completion contract. |
| Stop failure/shutdown require continued progress. | [`engine_cache` shutdown](../tests/shutdown.rs), [`engine_worker` shutdown](../examples/engine_worker/tests/shutdown.rs): pending ownership, delivery, escaped output and cleanup. | Controlled interleavings, not all application shutdown paths. |

The root test suite excludes the standalone `engine_worker` package. From the repository root:

```powershell
cargo test --locked
cargo test --locked --manifest-path examples/engine_worker/Cargo.toml
```

These tests use actual libraries. Full application workload scenarios and optimized
benchmark/flamegraph validation remain separate work.

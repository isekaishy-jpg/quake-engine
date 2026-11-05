fn main() {
    let report = engine_cache_worker_example::run_pipeline();
    engine_cache_worker_example::settle_saturated_attempt();
    engine_cache_worker_example::forward_ordered_demand();
    println!(
        "Published {:?}; retained retry, discovery, demand and owner cleanup settled.",
        report.bytes
    );
}

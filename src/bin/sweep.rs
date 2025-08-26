use copyforward::{fixture::generate_thread, CappedHashedGreedy, GreedySubstringConfig};
use std::time::Instant;

fn run_case(cap: usize, ncap: usize) {
    CappedHashedGreedy::set_overrides(Some(cap), Some(ncap));

    let msgs = generate_thread(42, 1000, 1000);
    let refs: Vec<&str> = msgs.iter().map(|s| s.as_str()).collect();

    let t0 = Instant::now();
    let c = CappedHashedGreedy::with_config(&GreedySubstringConfig::default(), &refs);
    let dur = t0.elapsed();
    let (km, tb, lk, ce, cc, ex) = copyforward::instrumentation::counters_snapshot();
    let (num,sum,max) = copyforward::instrumentation::lookup_stats_snapshot();
    let (we,wr) = copyforward::instrumentation::winner_stats_snapshot();

    println!("CAP={} NCAP={} build_time={:?} kmers={} table_ns={} lookups={} candidates={} chars={} ext_ns={} lookups_num={} mean_cand={} max_cand={} winner_ext={} winner_recovered={}",
        cap, ncap, dur, km, tb, lk, ce, cc, ex, num, if num>0 {sum/num} else {0}, max, we, wr);
}

fn main() {
    let caps = [32usize, 64, 128, 256];
    let ncaps = [32usize, 64, 128];
    for &cap in &caps {
        for &ncap in &ncaps {
            run_case(cap, ncap);
        }
    }
}

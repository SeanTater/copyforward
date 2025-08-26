use copyforward::{CopyForward, CappedHashedGreedy};
use std::time::Instant;
use copyforward::fixture::generate_thread;

fn run_case(msgs: &[&str]) {
    // Only run CappedHashedGreedy to focus profiling on the capped
    // implementation. This avoids time being spent in other algorithms.
    copyforward::instrumentation::reset_counters();
    let t = Instant::now();
    let c = CappedHashedGreedy::from_messages(msgs);
    let dur = t.elapsed();
    let (km, tb, lk, ce, cc, ex) = copyforward::instrumentation::counters_snapshot();
    println!(
        "CappedHashedGreedy: build_time={:?} kmers={} table_ns={} lookups={} candidates={} chars={} ext_ns={}",
        dur, km, tb, lk, ce, cc, ex
    );
    let (we, wr) = copyforward::instrumentation::winner_stats_snapshot();
    println!("CappedHashedGreedy: winner_exts={} winner_chars_recovered={}", we, wr);

    let _rc = CopyForward::render_with(&c, |_, _, _, s| s.to_string());
}

fn main() {
    let msgs = generate_thread(42, 2500, 2500);
    let refs: Vec<&str> = msgs.iter().map(|s| s.as_str()).collect();
    run_case(&refs);
}

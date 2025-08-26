use copyforward::{CopyForward, GreedySubstring, HashedGreedy, HashedGreedyBinary, CappedHashedGreedy};
use rand_chacha::ChaCha8Rng;
use std::time::Instant;
use copyforward::fixture::generate_thread;

fn run_case(msgs: &[&str]) {
    // (Removed) GreedySubstring: skip full greedy baseline to focus profiling on
    // capped/hashed implementations.

    // HashedGreedy
    copyforward::instrumentation::reset_counters();
    let t1 = Instant::now();
    let h = HashedGreedy::from_messages(msgs);
    let dur_h = t1.elapsed();
    let (km, tb, lk, ce, cc, ex) = copyforward::instrumentation::counters_snapshot();
    println!(
        "HashedGreedy: build_time={:?} kmers={} table_ns={} lookups={} candidates={} chars={} ext_ns={}",
        dur_h, km, tb, lk, ce, cc, ex
    );
    let (num,sum,max) = copyforward::instrumentation::lookup_stats_snapshot();
    if num > 0 {
        let mean = sum / num;
        println!("HashedGreedy: lookup_stats num={} mean_candidates={} max_candidates={}", num, mean, max);
    }

    // HashedGreedyBinary
    copyforward::instrumentation::reset_counters();
    let t2 = Instant::now();
    let hb = HashedGreedyBinary::from_messages(msgs);
    let dur_hb = t2.elapsed();
    let (km, tb, lk, ce, cc, ex) = copyforward::instrumentation::counters_snapshot();
    println!(
        "HashedGreedyBinary: build_time={:?} kmers={} table_ns={} lookups={} candidates={} chars={} ext_ns={}",
        dur_hb, km, tb, lk, ce, cc, ex
    );
    let (num,sum,max) = copyforward::instrumentation::lookup_stats_snapshot();
    if num > 0 {
        let mean = sum / num;
        println!("HashedGreedyBinary: lookup_stats num={} mean_candidates={} max_candidates={}", num, mean, max);
    }
    // bucket stats removed to keep instrumentation minimal

    // Render hashed variants to ensure no lazy work remains
    let _rh = CopyForward::render_with(&h, |_, _, _, s| s.to_string());
    let _rhb = CopyForward::render_with(&hb, |_, _, _, s| s.to_string());

    // CappedHashedGreedy
    copyforward::instrumentation::reset_counters();
    let t3 = Instant::now();
    let c = CappedHashedGreedy::from_messages(msgs);
    let dur_c = t3.elapsed();
    let (km, tb, lk, ce, cc, ex) = copyforward::instrumentation::counters_snapshot();
    println!(
        "CappedHashedGreedy: build_time={:?} kmers={} table_ns={} lookups={} candidates={} chars={} ext_ns={}",
        dur_c, km, tb, lk, ce, cc, ex
    );
    let (num,sum,max) = copyforward::instrumentation::lookup_stats_snapshot();
    if num > 0 {
        let mean = sum / num;
        println!("CappedHashedGreedy: lookup_stats num={} mean_candidates={} max_candidates={}", num, mean, max);
    }
    let (we, wr) = copyforward::instrumentation::winner_stats_snapshot();
    println!("CappedHashedGreedy: winner_exts={} winner_chars_recovered={}", we, wr);
    let (km, tb, lk, ce, cc, ex) = copyforward::instrumentation::counters_snapshot();
    println!("CappedHashedGreedy: final_counters kmers={} table_ns={} lookups={} candidates={} chars={} ext_ns={}", km, tb, lk, ce, cc, ex);

    let _rc = CopyForward::render_with(&c, |_, _, _, s| s.to_string());
}

fn main() {
    let msgs = generate_thread(42, 250, 250);
    let refs: Vec<&str> = msgs.iter().map(|s| s.as_str()).collect();
    run_case(&refs);
}

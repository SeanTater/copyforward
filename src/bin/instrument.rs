use std::time::Instant;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use copyforward::{GreedySubstring, HashedGreedy, CopyForward};

fn make_base_post(rng: &mut impl Rng, sentences: usize) -> String {
    let mut s = String::new();
    for i in 0..sentences {
        s.push_str(&format!("This is sentence {}.", i));
        if rng.gen_bool(0.5) { s.push(' '); }
    }
    s
}

fn generate_thread(rng: &mut impl Rng, n: usize, base_sentences: usize) -> Vec<String> {
    let mut messages: Vec<String> = Vec::with_capacity(n);
    let base = make_base_post(rng, base_sentences);
    messages.push(base.clone());

    for i in 1..n {
        let prev = messages[i-1].clone();
        let choice = rng.gen_range(0..3);
        let mut new_msg = match choice {
            0 => format!("{}\n> {}", prev, "Added at end."),
            1 => format!("Added at start.\n> {}", prev),
            _ => {
                let mid = prev.len()/2;
                let (a,b) = prev.split_at(mid);
                format!("{}{}{}", a, "\n[inline reply]\n", b)
            }
        };
        if rng.gen_bool(0.2) { new_msg.push_str(" Extra sentence."); }
        messages.push(new_msg);
    }

    messages
}

fn run_case(msgs: &[&str]) {
    // GreedySubstring
    copyforward::instrumentation::reset_counters();
    let t0 = Instant::now();
    let g = GreedySubstring::from_messages(msgs);
    let dur_g = t0.elapsed();
    let (km, tb, lk, ce, cc, ex) = copyforward::instrumentation::counters_snapshot();
    println!("GreedySubstring: build_time={:?} kmers={} table_ns={} lookups={} candidates={} chars={} ext_ns={}", dur_g, km, tb, lk, ce, cc, ex);

    // HashedGreedy
    copyforward::instrumentation::reset_counters();
    let t1 = Instant::now();
    let h = HashedGreedy::from_messages(msgs);
    let dur_h = t1.elapsed();
    let (km, tb, lk, ce, cc, ex) = copyforward::instrumentation::counters_snapshot();
    println!("HashedGreedy: build_time={:?} kmers={} table_ns={} lookups={} candidates={} chars={} ext_ns={}", dur_h, km, tb, lk, ce, cc, ex);

    // Render to ensure no lazy work remains
    let _rg = CopyForward::render_with(&g, |_, _, _, s| s.to_string());
    let _rh = CopyForward::render_with(&h, |_, _, _, s| s.to_string());
}

fn main() {
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let msgs = generate_thread(&mut rng, 250, 250);
    let refs: Vec<&str> = msgs.iter().map(|s| s.as_str()).collect();
    run_case(&refs);
}

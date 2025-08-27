use copyforward::{Config, CopyForward, approximate, exact};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

fn make_base_post(rng: &mut impl Rng, sentences: usize) -> String {
    let mut s = String::new();
    for i in 0..sentences {
        s.push_str(&format!("This is sentence {}.", i));
        if rng.gen_bool(0.5) {
            s.push(' ');
        }
    }
    s
}

/// Generate a thread of `n` messages where each message quotes the entire
/// previous message (transitively) and then randomly inserts text at the
/// start/end or in the middle.
fn generate_thread(rng: &mut impl Rng, n: usize, base_sentences: usize) -> Vec<String> {
    let mut messages: Vec<String> = Vec::with_capacity(n);
    let base = make_base_post(rng, base_sentences);
    messages.push(base.clone());

    for i in 1..n {
        let prev = messages[i - 1].clone();
        // Decide where to add: front, back, or inline
        let choice = rng.gen_range(0..3);
        let mut new_msg = match choice {
            0 => format!("{}\n> {}", prev, "Added at end."),
            1 => format!("Added at start.\n> {}", prev),
            _ => {
                // insert in middle roughly
                let mid = prev.len() / 2;
                let (a, b) = prev.split_at(mid);
                format!("{}{}{}", a, "\n[inline reply]\n", b)
            }
        };
        // occasionally small edit
        if rng.gen_bool(0.2) {
            new_msg.push_str(" Extra sentence.");
        }
        messages.push(new_msg);
    }

    messages
}

fn bench_algorithms(c: &mut Criterion) {
    let mut group = c.benchmark_group("copyforward_threaded");

    // parameters: number of messages and base size in sentences
    let message_counts = [100usize, 1000];
    let base_sentences = [100usize, 1000];

    for &msg_count in message_counts.iter() {
        for &base_s in base_sentences.iter() {
            let mut rng = ChaCha8Rng::seed_from_u64(42);
            let msgs = generate_thread(&mut rng, msg_count, base_s);
            let msg_refs: Vec<&str> = msgs.iter().map(|s| s.as_str()).collect();

            // Exact (binary-search extension)
            group.bench_with_input(
                BenchmarkId::from_parameter(format!("exact_msgs{}_base{}", msg_count, base_s)),
                &msg_refs,
                |b, m| {
                    b.iter(|| {
                        let cf = exact(m, Config::default());
                        // compute sizes
                        let orig: usize = m.iter().map(|s| s.len()).sum();
                        let segs = cf.segments();
                        let deduped: usize = segs
                            .iter()
                            .flat_map(|v| v.iter())
                            .map(|seg| match seg {
                                copyforward::Segment::Literal(s) => s.len(),
                                copyforward::Segment::Reference { .. } => 3, // replacement approx
                            })
                            .sum();
                        // ensure at least some deduping happened
                        assert!(deduped as f64 <= (orig as f64) * 0.95);
                    })
                },
            );

            // Approximate (cap + coalesce)
            group.bench_with_input(
                BenchmarkId::from_parameter(format!("capped_msgs{}_base{}", msg_count, base_s)),
                &msg_refs,
                |b, m| {
                    b.iter(|| {
                        let cf = approximate(m, Config::default());
                        // compute sizes
                        let orig: usize = m.iter().map(|s| s.len()).sum();
                        let segs = cf.segments();
                        let deduped: usize = segs
                            .iter()
                            .flat_map(|v| v.iter())
                            .map(|seg| match seg {
                                copyforward::Segment::Literal(s) => s.len(),
                                copyforward::Segment::Reference { .. } => 3, // replacement approx
                            })
                            .sum();
                        // ensure at least some deduping happened
                        assert!(deduped as f64 <= (orig as f64) * 0.95);
                    })
                },
            );
        }
    }

    group.finish();
}

criterion_group!(benches, bench_algorithms);
criterion_main!(benches);

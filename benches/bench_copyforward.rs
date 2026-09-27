//! Benchmarks for copyforward.
//!
//! Groups:
//! - `construct_text`: building compressors (`exact` / `approximate`) across
//!   workload shapes that stress different code paths.
//! - `construct_tokens`: the token-mode constructors on the same shapes.
//! - `ops_threaded`: post-construction operations (`segments`,
//!   `segments_chars`, rendering, and the `compression_ratio` composition)
//!   on a pre-built compressor, so their cost is measured separately from
//!   construction.
//!
//! Workload shapes:
//! - `threaded`: each message quotes the whole previous message (long
//!   matches; the fixture's default shape).
//! - `chat`: many short messages drawn from a phrase pool with a shared
//!   boilerplate tail (many medium matches, large candidate buckets).
//! - `repetitive`: one patterned message repeated (intra-message k-mer
//!   collisions, dense buckets).
//! - `unique`: random low-repetition text (mostly the literal-scan path).

use std::time::{Duration, Instant};

use copyforward::{
    Config, CopyForward, CopyForwardTokens, approximate, approximate_tokens, exact, exact_tokens,
    fixture::generate_thread,
};
use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
    measurement::WallTime,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Calibrate with untimed iterations and size the group's sampling budget
/// so fast cases finish quickly and slow ones still collect a solid sample
/// count (and do not warn).
fn tune_group(group: &mut BenchmarkGroup<'_, WallTime>, calibrations: Vec<Duration>) {
    let per = calibrations
        .into_iter()
        .max()
        .unwrap_or_else(|| Duration::from_secs(1));
    let n = (Duration::from_secs(8).as_nanos() / per.as_nanos().max(1)).clamp(20, 100) as usize;
    group.sample_size(n);
    group.measurement_time(per * n as u32 * 3 / 2);
}

fn time_once<F: FnOnce() -> R, R>(f: F) -> Duration {
    let start = Instant::now();
    let _ = f();
    start.elapsed()
}

fn chat_thread(seed: u64, n_msgs: usize) -> Vec<String> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    // A shared boilerplate tail guarantees medium-length matches between
    // any two messages that draw the same phrase.
    let pool: Vec<String> = (0..100)
        .map(|i| format!("phrase number {i} with shared tail text"))
        .collect();
    (0..n_msgs)
        .map(|_| {
            let k = rng.gen_range(1..=3);
            let mut s = String::new();
            for _ in 0..k {
                s.push_str(&pool[rng.gen_range(0..pool.len())]);
                s.push(' ');
            }
            s
        })
        .collect()
}

fn repetitive_thread(n_msgs: usize, chars: usize) -> Vec<String> {
    let base: String = (0..chars)
        .map(|i| char::from(b'a' + (i % 23) as u8))
        .collect();
    vec![base; n_msgs]
}

fn unique_thread(seed: u64, n_msgs: usize, chars: usize) -> Vec<String> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    (0..n_msgs)
        .map(|_| {
            (0..chars)
                .map(|_| char::from(rng.gen_range(32u8..=126)))
                .collect()
        })
        .collect()
}

fn to_tokens(msgs: &[String]) -> Vec<Vec<u32>> {
    msgs.iter()
        .map(|s| s.chars().map(|c| c as u32).collect())
        .collect()
}

fn compressed_size(segs: &[Vec<copyforward::Segment>]) -> usize {
    segs.iter()
        .flat_map(|v| v.iter())
        .map(|seg| match seg {
            copyforward::Segment::Literal(s) => s.len(),
            copyforward::Segment::Reference { .. } => 3,
        })
        .sum()
}

fn bench_construct_text(c: &mut Criterion) {
    let cases: Vec<(&str, Vec<String>, bool)> = vec![
        ("threaded_small", generate_thread(42, 100, 100), true),
        ("threaded_large", generate_thread(42, 500, 100), true),
        ("chat_small", chat_thread(7, 2000), true),
        ("chat_large", chat_thread(7, 10000), true),
        ("repetitive_small", repetitive_thread(50, 1000), true),
        ("repetitive_large", repetitive_thread(500, 1000), true),
        ("unique_small", unique_thread(99, 500, 400), false),
        ("unique_large", unique_thread(99, 5000, 400), false),
    ];

    for (name, msgs, expect_ratio) in &cases {
        let refs: Vec<&str> = msgs.iter().map(String::as_str).collect();
        let bytes: usize = msgs.iter().map(|s| s.len()).sum();
        let config = Config::default();

        // Correctness and quality checks run once, outside the timing loop.
        let cf = exact(&refs, config.clone());
        assert_eq!(
            cf.render_with(|_, _, _, t| t.to_string()),
            *msgs,
            "exact round-trip failed for {name}"
        );
        let cf = approximate(&refs, config.clone());
        assert_eq!(
            cf.render_with(|_, _, _, t| t.to_string()),
            *msgs,
            "approximate round-trip failed for {name}"
        );
        if *expect_ratio {
            let deduped = compressed_size(&cf.segments());
            assert!(
                deduped as f64 <= bytes as f64 * 0.95,
                "expected compression for {name}: deduped={deduped} bytes={bytes}"
            );
        }

        let mut group = c.benchmark_group(format!("construct_text_{name}"));
        group.throughput(Throughput::Bytes(bytes as u64));
        tune_group(
            &mut group,
            vec![
                time_once(|| exact(&refs, config.clone())),
                time_once(|| approximate(&refs, config.clone())),
            ],
        );
        group.bench_function("exact", |b| b.iter(|| exact(&refs, config.clone())));
        group.bench_function("approximate", |b| {
            b.iter(|| approximate(&refs, config.clone()))
        });
        group.finish();
    }
}

fn bench_construct_tokens(c: &mut Criterion) {
    let cases: Vec<(&str, Vec<Vec<u32>>)> = {
        let text_cases: Vec<(&str, Vec<String>)> = vec![
            ("threaded_small", generate_thread(42, 100, 100)),
            ("threaded_large", generate_thread(42, 500, 100)),
            ("chat_small", chat_thread(7, 2000)),
            ("chat_large", chat_thread(7, 10000)),
        ];
        text_cases
            .into_iter()
            .map(|(name, msgs)| (name, to_tokens(&msgs)))
            .collect()
    };

    for (name, toks) in &cases {
        let refs: Vec<&[u32]> = toks.iter().map(|v| v.as_slice()).collect();
        let bytes: usize = toks
            .iter()
            .map(|v| v.len() * std::mem::size_of::<u32>())
            .sum();
        let config = Config::default();

        // Round-trip checks once, outside the timing loop.
        let cf = exact_tokens(&refs, config.clone());
        assert_eq!(
            cf.render_with(|_, _, _, s| s.to_vec()),
            *toks,
            "exact token round-trip {name}"
        );
        let cf = approximate_tokens(&refs, config.clone());
        assert_eq!(
            cf.render_with(|_, _, _, s| s.to_vec()),
            *toks,
            "approximate token round-trip {name}"
        );

        let mut group = c.benchmark_group(format!("construct_tokens_{name}"));
        group.throughput(Throughput::Bytes(bytes as u64));
        tune_group(
            &mut group,
            vec![
                time_once(|| exact_tokens(&refs, config.clone())),
                time_once(|| approximate_tokens(&refs, config.clone())),
            ],
        );
        group.bench_function("exact", |b| b.iter(|| exact_tokens(&refs, config.clone())));
        group.bench_function("approximate", |b| {
            b.iter(|| approximate_tokens(&refs, config.clone()))
        });
        group.finish();
    }
}

fn bench_ops_for<C: CopyForward>(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    name: &str,
    cf: &C,
) {
    group.bench_with_input(BenchmarkId::new(name, "segments"), cf, |b, cf| {
        b.iter(|| cf.segments())
    });
    group.bench_with_input(BenchmarkId::new(name, "segments_chars"), cf, |b, cf| {
        b.iter(|| cf.segments_chars())
    });
    group.bench_with_input(BenchmarkId::new(name, "render_static"), cf, |b, cf| {
        b.iter(|| cf.render_with_static("[R]"))
    });
    group.bench_with_input(BenchmarkId::new(name, "render_identity"), cf, |b, cf| {
        b.iter(|| cf.render_with(|_, _, _, t| t.to_string()))
    });
    // What CopyForwardText.compression_ratio() does internally.
    group.bench_with_input(BenchmarkId::new(name, "ratio_composition"), cf, |b, cf| {
        b.iter(|| {
            let segs = cf.segments();
            let rendered = cf.render_with_static("");
            (compressed_size(&segs), rendered.len())
        })
    });
}

fn bench_ops(c: &mut Criterion) {
    let msgs = generate_thread(42, 500, 100);
    let refs: Vec<&str> = msgs.iter().map(String::as_str).collect();
    let config = Config::default();
    let exact_cf = exact(&refs, config.clone());
    let approx_cf = approximate(&refs, config);

    let mut group = c.benchmark_group("ops_threaded");
    // The slowest op (ratio_composition) is ~10x the fastest; 60 samples
    // keeps every op inside the default measurement budget.
    group.sample_size(60);
    bench_ops_for(&mut group, "exact", &exact_cf);
    bench_ops_for(&mut group, "approximate", &approx_cf);
    group.finish();
}

criterion_group!(
    benches,
    bench_construct_text,
    bench_construct_tokens,
    bench_ops
);
criterion_main!(benches);

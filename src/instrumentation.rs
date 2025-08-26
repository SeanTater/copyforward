// Lightweight instrumentation for counting hotspots in development.
// Uses atomics to avoid locking overhead; reset and snapshot helpers
// allow a small dev binary to collect simple breakdowns.
use std::cell::Cell;

thread_local! {
    static KMERS_INSERTED: Cell<u64> = Cell::new(0);
    static TABLE_BUILD_NS: Cell<u64> = Cell::new(0);
    static LOOKUP_COUNT: Cell<u64> = Cell::new(0);
    static CANDIDATES_EXAMINED: Cell<u64> = Cell::new(0);
    static CHARS_COMPARED: Cell<u64> = Cell::new(0);
    static EXTENSION_NS: Cell<u64> = Cell::new(0);
    // kept minimal: avoid large histograms in hot path
    static SUM_CANDIDATES: Cell<u64> = Cell::new(0);
    static NUM_LOOKUPS: Cell<u64> = Cell::new(0);
    static MAX_CANDIDATES: Cell<u64> = Cell::new(0);
    static WINNER_EXTENSIONS: Cell<u64> = Cell::new(0);
    static WINNER_CHARS_RECOVERED: Cell<u64> = Cell::new(0);
}

pub fn reset_counters() {
    KMERS_INSERTED.with(|c| c.set(0));
    TABLE_BUILD_NS.with(|c| c.set(0));
    LOOKUP_COUNT.with(|c| c.set(0));
    CANDIDATES_EXAMINED.with(|c| c.set(0));
    CHARS_COMPARED.with(|c| c.set(0));
    EXTENSION_NS.with(|c| c.set(0));
    SUM_CANDIDATES.with(|c| c.set(0));
    NUM_LOOKUPS.with(|c| c.set(0));
    MAX_CANDIDATES.with(|c| c.set(0));
    WINNER_EXTENSIONS.with(|c| c.set(0));
    WINNER_CHARS_RECOVERED.with(|c| c.set(0));
}


pub fn counters_snapshot() -> (u64, u64, u64, u64, u64, u64) {
    let km = KMERS_INSERTED.with(|c| c.get());
    let tb = TABLE_BUILD_NS.with(|c| c.get());
    let lk = LOOKUP_COUNT.with(|c| c.get());
    let ce = CANDIDATES_EXAMINED.with(|c| c.get());
    let cc = CHARS_COMPARED.with(|c| c.get());
    let ex = EXTENSION_NS.with(|c| c.get());
    (km, tb, lk, ce, cc, ex)
}

pub fn add_duration_table_build_ns(n: u64) { add_table_build_ns(n); }


pub fn add_lookup_count(n_candidates: usize) {
    let n64 = n_candidates as u64;
    SUM_CANDIDATES.with(|c| c.set(c.get().wrapping_add(n64)));
    NUM_LOOKUPS.with(|c| c.set(c.get().wrapping_add(1)));
    MAX_CANDIDATES.with(|c| c.set(std::cmp::max(c.get(), n64)));
}

pub fn lookup_stats_snapshot() -> (u64, u64, u64) {
    let num = NUM_LOOKUPS.with(|c| c.get());
    let sum = SUM_CANDIDATES.with(|c| c.get());
    let max = MAX_CANDIDATES.with(|c| c.get());
    (num, sum, max)
}

pub fn add_winner_extension(n: u64) {
    WINNER_EXTENSIONS.with(|c| c.set(c.get().wrapping_add(n)));
}

pub fn add_winner_chars_recovered(n: u64) {
    WINNER_CHARS_RECOVERED.with(|c| c.set(c.get().wrapping_add(n)));
}

pub fn winner_stats_snapshot() -> (u64, u64) {
    let e = WINNER_EXTENSIONS.with(|c| c.get());
    let r = WINNER_CHARS_RECOVERED.with(|c| c.get());
    (e, r)
}

pub fn add_kmers(n: u64) {
    KMERS_INSERTED.with(|c| c.set(c.get().wrapping_add(n)));
}
pub fn add_table_build_ns(n: u64) {
    TABLE_BUILD_NS.with(|c| c.set(c.get().wrapping_add(n)));
}

// For the capped implementation we reuse a counter to record how many
// insertions were skipped due to identical cap-window hashes. This is a
// transient diagnostic, so we store it in TABLE_BUILD_NS too (added above).
pub fn add_lookup(n: u64) {
    LOOKUP_COUNT.with(|c| c.set(c.get().wrapping_add(n)));
}
pub fn add_candidates(n: u64) {
    CANDIDATES_EXAMINED.with(|c| c.set(c.get().wrapping_add(n)));
}

// kept minimal; additional ad-hoc diagnostics can be added in the instrument binary
pub fn add_chars(n: u64) {
    CHARS_COMPARED.with(|c| c.set(c.get().wrapping_add(n)));
}
pub fn add_extension_ns(n: u64) {
    EXTENSION_NS.with(|c| c.set(c.get().wrapping_add(n)));
}

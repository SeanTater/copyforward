use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Generate a deterministic thread of messages for testing/benchmarking.
pub fn generate_thread(seed: u64, n: usize, base_sentences: usize) -> Vec<String> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    fn make_base_post(rng: &mut impl Rng, sentences: usize) -> String {
        let mut s = String::new();
        for i in 0..sentences {
            s.push_str(&format!("This is sentence {i}."));
            if rng.gen_bool(0.5) {
                s.push(' ');
            }
        }
        s
    }

    let mut messages: Vec<String> = Vec::with_capacity(n);
    let base = make_base_post(&mut rng, base_sentences);
    messages.push(base.clone());

    for i in 1..n {
        let prev = messages[i - 1].clone();
        let choice = rng.gen_range(0..3);
        let mut new_msg = match choice {
            0 => format!("{prev}\n> Added at end."),
            1 => format!("Added at start.\n> {prev}"),
            _ => {
                let mid = prev.len() / 2;
                let (a, b) = prev.split_at(mid);
                format!("{}{}{}", a, "\n[inline reply]\n", b)
            }
        };
        if rng.gen_bool(0.2) {
            new_msg.push_str(" Extra sentence.");
        }
        messages.push(new_msg);
    }

    messages
}

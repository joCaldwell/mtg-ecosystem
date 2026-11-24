use super::Block;
use rand_chacha::ChaCha20Rng;
use rand_core::{Rng, SeedableRng};
use serde_json::{Value, json};

#[derive(Default)]
struct Sums {
    full: i64,
    full_count: usize,
    complete: i64,
    complete_count: usize,
    censor_difference: i64,
}
fn ratio(value: i64, n: usize) -> Option<f64> {
    (n != 0).then(|| value as f64 / n as f64)
}
impl Sums {
    fn add(&mut self, b: Block) {
        if b.complete_pairs == 4 {
            self.full += i64::from(b.difference);
            self.full_count += 4;
        }
        self.complete += i64::from(b.difference);
        self.complete_count += b.complete_pairs;
        self.censor_difference += i64::from(b.censored[1] - b.censored[0]);
    }
}
fn interval(mut draws: Vec<f64>) -> Option<[f64; 2]> {
    if draws.is_empty() {
        return None;
    }
    draws.sort_unstable_by(f64::total_cmp);
    let percentile = |p: usize| draws[(p * draws.len()).div_ceil(1000) - 1];
    Some([percentile(25), percentile(975)])
}
// Sum the two-sided Binomial(n, 1/2) tail using adjacent mass ratios,
// normalized at the mode so that large n does not underflow at the origin.
fn sign_test(pos: usize, neg: usize) -> f64 {
    let n = pos + neg;
    let mode = n / 2;
    let cutoff = pos.min(neg);
    if cutoff == mode {
        return 1.;
    }
    let mut total = if n.is_multiple_of(2) { 1. } else { 2. };
    let mut tail = 0.;
    let mut mass = 1.;
    for k in (1..=mode).rev() {
        mass *= k as f64 / (n - k + 1) as f64;
        total += 2. * mass;
        if k <= cutoff + 1 {
            tail += 2. * mass;
        }
    }
    tail / total
}
pub(super) fn report(
    data: &[Block],
    table: [[usize; 2]; 2],
    invalid: bool,
    seed: [u8; 32],
) -> Value {
    let n = data.len();
    let mut sum = Sums::default();
    let (mut pos, mut neg) = (0, 0);
    let (mut low, mut high) = (0i64, 0i64);
    for &b in data {
        sum.add(b);
        low += i64::from(b.lower);
        high += i64::from(b.upper);
        if b.complete_pairs == 4 {
            pos += usize::from(b.difference > 0);
            neg += usize::from(b.difference < 0);
        }
    }
    let mut rng = ChaCha20Rng::from_seed(seed);
    let mut full = Vec::new();
    let mut complete = Vec::new();
    let mut censor = Vec::new();
    let bound = n as u64;
    let threshold = bound.wrapping_neg() % bound;
    for _ in 0..10_000 {
        let mut draw = Sums::default();
        for _ in 0..n {
            let word = loop {
                let w = rng.next_u64();
                if w >= threshold {
                    break w;
                }
            };
            draw.add(data[(word % bound) as usize]);
        }
        full.extend(ratio(draw.full, draw.full_count));
        complete.extend(ratio(draw.complete, draw.complete_count));
        censor.push(draw.censor_difference as f64 / (4 * n) as f64);
    }
    let available = [full.len(), complete.len()];
    let full_ci = interval(full);
    let cc_ci = interval(complete);
    let censor_ci = interval(censor).expect("nonempty block draws");
    let excludes_zero = |[l, h]: [f64; 2]| l > 0. || h < 0.;
    let rates = [
        (table[1][0] + table[1][1]) as f64 / (4 * n) as f64,
        (table[0][1] + table[1][1]) as f64 / (4 * n) as f64,
    ];
    let confounded = rates.iter().any(|&r| r > 0.02)
        || excludes_zero(censor_ci)
        || (low <= 0 && high >= 0 && cc_ci.is_some_and(excludes_zero));
    json!({"estimate":ratio(sum.full,sum.full_count),"ci95":full_ci,"complete_blocks":sum.full_count/4,
        "complete_case_estimate":ratio(sum.complete,sum.complete_count),"complete_case_ci95":cc_ci,"complete_pairs":sum.complete_count,
        "worst_case_bounds":[ratio(low,4*n),ratio(high,4*n)],"censoring_table":table,"censoring_rates":rates,
        "censoring_difference":ratio(sum.censor_difference,4*n),"censoring_difference_ci95":censor_ci,
        "sign_test":{"positive":pos,"negative":neg,"two_sided_p":sign_test(pos,neg)},
        "bootstrap_draws":10000,"defined_bootstrap_draws":available,"verdict":if invalid {"invalid"} else if confounded {"confounded"} else {"valid"}})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn censoring_reports_missing_outcomes_and_faults_take_precedence() {
        let b = Block {
            complete_pairs: 3,
            difference: 1,
            censored: [1, 0],
            lower: 0,
            upper: 1,
        };
        let r = report(&[b], [[3, 0], [1, 0]], false, [0; 32]);
        assert!(r["estimate"].is_null());
        assert_eq!(r["complete_case_estimate"], 1. / 3.);
        assert_eq!(r["censoring_difference_ci95"], json!([-0.25, -0.25]));
        assert_eq!(r["worst_case_bounds"], json!([0., 0.25]));
        assert_eq!(r["verdict"], "confounded");
        assert_eq!(
            report(&[b], [[3, 0], [1, 0]], true, [0; 32])["verdict"],
            "invalid"
        );
    }
    #[test]
    fn bootstrap_is_deterministic_and_uses_whole_blocks() {
        let data = [
            Block {
                complete_pairs: 4,
                difference: 4,
                lower: 4,
                upper: 4,
                ..Default::default()
            },
            Block {
                complete_pairs: 4,
                difference: -4,
                lower: -4,
                upper: -4,
                ..Default::default()
            },
        ];
        let run = || report(&data, [[8, 0], [0, 0]], false, [17; 32]);
        let r = run();
        assert_eq!(r, run());
        assert_eq!(r["ci95"], json!([-1., 1.]));
        assert_eq!(r["estimate"], 0.);
        assert_eq!(sign_test(5, 0), 0.0625);
        assert_eq!(sign_test(0, 0), 1.);
    }
}

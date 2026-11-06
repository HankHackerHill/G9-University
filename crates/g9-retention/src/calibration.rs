//! Confidence calibration over stated probabilities (handoff §10, LRN-xx):
//! Brier score and reliability bins. A 0–5 self-rating is NOT a probability;
//! any such scale must be mapped through an explicitly validated table before
//! it reaches these functions.

use g9_domain::{Confidence, EvidenceId, UtcMillis};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationObservation {
    pub question_ref: EvidenceId,
    /// Stated probability of being correct, in [0, 1].
    pub stated_probability: Confidence,
    pub correct: bool,
    pub at: UtcMillis,
}

/// Mean Brier score; `None` for an empty set. 0 is perfect, 0.25 is the score of
/// always saying 0.5, 1 is maximally wrong.
pub fn brier_score(obs: &[CalibrationObservation]) -> Option<f64> {
    if obs.is_empty() {
        return None;
    }
    let sum: f64 = obs
        .iter()
        .map(|o| o.stated_probability.brier(o.correct))
        .sum();
    Some(sum / obs.len() as f64)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReliabilityBin {
    pub lo: f64,
    pub hi: f64,
    pub n: usize,
    pub mean_confidence: Option<f64>,
    pub hit_rate: Option<f64>,
}

/// Equal-width reliability bins over [0, 1]; the last bin is closed on 1.0.
/// The bin counts sum to `obs.len()`.
pub fn reliability_bins(obs: &[CalibrationObservation], n_bins: usize) -> Vec<ReliabilityBin> {
    let n_bins = n_bins.max(1);
    let width = 1.0 / n_bins as f64;
    let mut sums = vec![(0usize, 0.0f64, 0usize); n_bins];
    for o in obs {
        let p = o.stated_probability.get();
        let idx = ((p / width).floor() as usize).min(n_bins - 1);
        let b = &mut sums[idx];
        b.0 += 1;
        b.1 += p;
        if o.correct {
            b.2 += 1;
        }
    }
    sums.into_iter()
        .enumerate()
        .map(|(i, (n, sum_p, hits))| ReliabilityBin {
            lo: i as f64 * width,
            hi: if i + 1 == n_bins {
                1.0
            } else {
                (i + 1) as f64 * width
            },
            n,
            mean_confidence: if n > 0 { Some(sum_p / n as f64) } else { None },
            hit_rate: if n > 0 {
                Some(hits as f64 / n as f64)
            } else {
                None
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(p: f64, correct: bool) -> CalibrationObservation {
        CalibrationObservation {
            question_ref: EvidenceId::parse("q").unwrap(),
            stated_probability: Confidence::new(p).unwrap(),
            correct,
            at: UtcMillis::from_millis(0),
        }
    }

    #[test]
    fn brier_known_values() {
        assert_eq!(brier_score(&[]), None);
        assert_eq!(brier_score(&[obs(1.0, true), obs(0.0, false)]), Some(0.0));
        let mixed = vec![
            obs(0.5, true),
            obs(0.5, false),
            obs(0.5, true),
            obs(0.5, false),
        ];
        assert!((brier_score(&mixed).unwrap() - 0.25).abs() < 1e-12);
        assert_eq!(brier_score(&[obs(1.0, false)]), Some(1.0));
    }

    #[test]
    fn reliability_bins_sum_to_n() {
        let data: Vec<_> = (0..=20).map(|i| obs(i as f64 / 20.0, i % 3 == 0)).collect();
        let bins = reliability_bins(&data, 5);
        assert_eq!(bins.len(), 5);
        assert_eq!(bins.iter().map(|b| b.n).sum::<usize>(), data.len());
        assert_eq!(bins[4].hi, 1.0);
        assert!(bins[4].n >= 1, "p = 1.0 lands in the last bin");
        let empty = reliability_bins(&[], 4);
        assert!(empty.iter().all(|b| b.n == 0 && b.hit_rate.is_none()));
    }
}

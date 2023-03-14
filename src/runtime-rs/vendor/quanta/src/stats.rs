/// Estimates the arithmetic mean (and the error) for a set of samples.
///
/// This type is written and maintained internally as it is trivial to implement and doesn't
/// warrant a separate dependency.  As well, we add some features like exposing the sample count,
/// calculating the mean + error value, etc, that existing crates don't do.
///
/// Based on Welford's algorithm: computes the mean incrementally, with constant time and
/// space complexity.
///
/// https://en.wikipedia.org/wiki/Algorithms_for_calculating_variance#Welford's_online_algorithm
#[derive(Default)]
pub(crate) struct Variance {
    mean: f64,
    n: u64,
    sum_sq: f64,
}

impl Variance {
    #[inline]
    pub fn add(&mut self, sample: f64) {
        self.n += 1;
        let n_f = self.n as f64;
        let delta = (sample - self.mean) / self.n as f64;
        self.mean += delta;
        self.sum_sq += delta * delta * n_f * (n_f - 1.0);
    }

    #[inline]
    pub fn mean(&self) -> f64 {
        self.mean
    }

    #[inline]
    pub fn mean_error(&self) -> f64 {
        if self.n < 2 {
            return 0.0;
        }

        let n_f = self.n as f64;
        ((self.sum_sq / (n_f - 1.0)) / n_f).sqrt()
    }

    #[inline]
    pub fn mean_with_error(&self) -> u64 {
        let mean = self.mean.abs();
        let total = mean + self.mean_error().abs();
        total as u64
    }

    #[inline]
    pub fn has_significant_result(&self) -> bool {
        self.n >= 2
    }

    #[inline]
    pub fn samples(&self) -> u64 {
        self.n
    }
}

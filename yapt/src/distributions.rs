use rand::Rng;

#[derive(Debug, Clone, PartialEq)]
pub struct Distribution1D {
    pub pdf: Vec<f32>,
    pub cdf: Vec<f32>,
    pub func_int: f32,
}

impl Distribution1D {
    #[must_use]
    pub fn new(values: &[f32]) -> Self {
        assert!(
            !values.is_empty(),
            "Empty slice passed to Distribution1D::new!"
        );

        let n = values.len();

        let mut intervals = vec![0.0];

        for i in 1..=n {
            let last = intervals[i - 1];
            intervals.push(last + values[i - 1]);
        }

        let func_int = intervals[n];
        for value in &mut intervals {
            if func_int != 0.0 {
                *value /= func_int;
            }
        }

        let mut pdf = Vec::new();
        let mut last = 0.0;
        for value in &intervals[1..] {
            pdf.push(value - last);
            last = *value;
        }

        Self {
            pdf,
            cdf: intervals,
            func_int,
        }
    }

    pub fn sample_naive(&self, rng: &mut impl Rng) -> usize {
        let threshold = rng.random();

        self.cdf.iter().position(|v| v >= &threshold).unwrap() - 1
    }
    pub fn sample(&self, rng: &mut impl Rng) -> usize {
        let num = rng.random();

        let pred = |i| self.cdf[i] <= num;

        {
            let mut first = 0;
            let mut len = self.cdf.len();
            while len > 0 {
                let half = len >> 1;
                let middle = first + half;

                if pred(middle) {
                    first = middle + 1;
                    len -= half + 1;
                } else {
                    len = half;
                }
            }
            (first - 1).clamp(0, self.cdf.len() - 2)
        }
    }
}

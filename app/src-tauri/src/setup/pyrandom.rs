//! Python's `random.Random` (Mersenne Twister, seeded from an int), so files the
//! Python installer made from a seed (the CS 1.6 bot profile, the bot styles)
//! come out byte for byte the same here.

pub struct PyRandom {
    mt: [u32; 624],
    idx: usize,
}

impl PyRandom {
    /// random.Random(seed) for a non-negative int seed.
    pub fn new(seed: u64) -> PyRandom {
        // the seed's 32-bit words, least significant first ([0] for 0)
        let mut key: Vec<u32> = Vec::new();
        let mut n = seed;
        while n > 0 {
            key.push(n as u32);
            n >>= 32;
        }
        if key.is_empty() {
            key.push(0);
        }
        let mut r = PyRandom { mt: [0; 624], idx: 624 };
        r.init_genrand(19_650_218);
        let (mut i, mut j) = (1usize, 0usize);
        let mut k = 624.max(key.len());
        while k > 0 {
            let prev = r.mt[i - 1] ^ (r.mt[i - 1] >> 30);
            r.mt[i] = (r.mt[i] ^ prev.wrapping_mul(1_664_525)).wrapping_add(key[j]).wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= 624 {
                r.mt[0] = r.mt[623];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
            k -= 1;
        }
        k = 623;
        while k > 0 {
            let prev = r.mt[i - 1] ^ (r.mt[i - 1] >> 30);
            r.mt[i] = (r.mt[i] ^ prev.wrapping_mul(1_566_083_941)).wrapping_sub(i as u32);
            i += 1;
            if i >= 624 {
                r.mt[0] = r.mt[623];
                i = 1;
            }
            k -= 1;
        }
        r.mt[0] = 0x8000_0000;
        r
    }

    fn init_genrand(&mut self, s: u32) {
        self.mt[0] = s;
        for i in 1..624 {
            self.mt[i] = 1_812_433_253u32.wrapping_mul(self.mt[i - 1] ^ (self.mt[i - 1] >> 30)).wrapping_add(i as u32);
        }
        self.idx = 624;
    }

    fn next_u32(&mut self) -> u32 {
        const UPPER: u32 = 0x8000_0000;
        const LOWER: u32 = 0x7fff_ffff;
        const MATRIX_A: u32 = 0x9908_b0df;
        if self.idx >= 624 {
            for kk in 0..624 {
                let y = (self.mt[kk] & UPPER) | (self.mt[(kk + 1) % 624] & LOWER);
                self.mt[kk] = self.mt[(kk + 397) % 624] ^ (y >> 1) ^ if y & 1 != 0 { MATRIX_A } else { 0 };
            }
            self.idx = 0;
        }
        let mut y = self.mt[self.idx];
        self.idx += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// random.random(): a float in [0, 1) with 53 random bits.
    pub fn random(&mut self) -> f64 {
        let a = (self.next_u32() >> 5) as f64;
        let b = (self.next_u32() >> 6) as f64;
        (a * 67_108_864.0 + b) * (1.0 / 9_007_199_254_740_992.0)
    }

    fn getrandbits(&mut self, k: u32) -> u32 {
        debug_assert!((1..=32).contains(&k));
        self.next_u32() >> (32 - k)
    }

    /// random._randbelow(n) for 0 < n < 2^32.
    fn randbelow(&mut self, n: u32) -> u32 {
        let k = 32 - n.leading_zeros();
        loop {
            let r = self.getrandbits(k);
            if r < n {
                return r;
            }
        }
    }

    /// random.randint(a, b)
    pub fn randint(&mut self, a: i64, b: i64) -> i64 {
        a + self.randbelow((b - a + 1) as u32) as i64
    }

    /// Index chosen by random.choice(seq) for a seq of length n.
    pub fn choice(&mut self, n: usize) -> usize {
        self.randbelow(n as u32) as usize
    }

    /// Index chosen by random.choices(population, weights)[0].
    pub fn choices_weighted(&mut self, weights: &[f64]) -> usize {
        let mut cum = Vec::with_capacity(weights.len());
        let mut t = 0.0;
        for w in weights {
            t += w;
            cum.push(t);
        }
        let total = t + 0.0;
        let x = self.random() * total;
        // bisect_right(cum, x, 0, n - 1)
        let (mut lo, mut hi) = (0usize, weights.len() - 1);
        while lo < hi {
            let mid = (lo + hi) / 2;
            if x < cum[mid] {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        lo
    }

    /// random.uniform(a, b)
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }
}

#[cfg(test)]
mod tests {
    use super::PyRandom;

    #[test]
    fn matches_python_known_values() {
        // python3 -c "import random; r=random.Random(2004); print(repr(r.random()), r.randint(-20,20), r.randint(40,100))"
        let Some(out) = super::super::test_python(
            "import random; r=random.Random(2004); print(repr(r.random()), r.randint(-20,20), r.randint(40,100), r.choice([1,2,3,4,5,6]), r.choices(range(7),[5,4,2,2,1,1,2])[0], repr(r.uniform(-5,5)))",
        ) else {
            return;
        };
        let mut r = PyRandom::new(2004);
        let mine = format!(
            "{} {} {} {} {} {}",
            super::super::py_repr_float(r.random()),
            r.randint(-20, 20),
            r.randint(40, 100),
            r.choice(6) + 1,
            r.choices_weighted(&[5.0, 4.0, 2.0, 2.0, 1.0, 1.0, 2.0]),
            super::super::py_repr_float(r.uniform(-5.0, 5.0))
        );
        assert_eq!(mine, out.trim());
    }

    #[test]
    fn big_seed() {
        let Some(out) = super::super::test_python("import random; r=random.Random(2**40+7); print(repr(r.random()))") else { return };
        let mut r = PyRandom::new((1u64 << 40) + 7);
        assert_eq!(super::super::py_repr_float(r.random()), out.trim());
    }
}

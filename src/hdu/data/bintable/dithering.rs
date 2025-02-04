const N_RANDOM: usize = 10000;

/// Random generator given in annexe of the
/// Tiles Image Convention for Storing Compressed Images in FITS Binary Tables
pub(crate) const fn random_generator() -> [f32; N_RANDOM] {
    let a: f64 = 16807.0;
    let m: f64 = 2147483647.0;
    let mut seed: f64 = 1.0;

    let mut rand_value = [0.0_f32; N_RANDOM];

    let mut i = 0;
    while i < N_RANDOM {
        let temp = a * seed;
        seed = temp - m * (((temp / m) as i32) as f64);
        rand_value[i] = (seed / m) as f32;

        i += 1;
    }

    rand_value
}

pub(crate) const RAND_VALUES: [f32; N_RANDOM] = random_generator();

mod tests {
    #[test]
    fn test_random_generator() {
        super::random_generator();
    }
}


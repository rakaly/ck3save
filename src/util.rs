pub(crate) fn reencode_float(f: f64) -> f64 {
    // first reverse the flavor decoding to get raw val
    let f = f * 1000.0;

    // Then apply the eu4 decoding step (Q49.15) with 5 digits of precision.
    // For some unknown reason we need to incorporate float epsilon so that a
    // number decoded as 251.24999999999 should be decoded as 251.25000 and
    // using 32 bits epsilon was the only way I could get equivalent plaintext
    // and binary saves to agree on reencoded float values.
    let eps = f64::from(f32::EPSILON);
    let num = (f / 32768.0 * 100_000.0 + (eps * f.signum())).trunc();
    num / 100_000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use jomini::{BinaryFlavor, Ck3Flavor};

    #[test]
    fn reencode_accuracy() {
        let data: [u8; 8] = [0, 160, 125, 0, 0, 0, 0, 0];
        let flavor = Ck3Flavor::default();
        let f = flavor.visit_f64(data);
        let newf = reencode_float(f);
        assert_eq!(newf, 251.25000);
    }

    #[test]
    fn reencode_accuracy_2() {
        let data: [u8; 8] = [6, 193, 0, 0, 0, 0, 0, 0];
        let flavor = Ck3Flavor::default();
        let f = flavor.visit_f64(data);
        let newf = reencode_float(f);
        assert_eq!(newf, 1.50799);
    }

    #[test]
    fn reencode_accuracy_3() {
        let data: [u8; 8] = [0, 0, 81, 255, 255, 255, 255, 255];
        let flavor = Ck3Flavor::default();
        let f = flavor.visit_f64(data);
        let newf = reencode_float(f);
        assert_eq!(newf, -350.0);
    }
}

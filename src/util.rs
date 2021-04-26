pub(crate) fn reencode_float(f: f64) -> f64 {
    // first reverse the flavor decoding to get raw val
    let f = f * 1000.0;

    // Then apply the eu4 decoding step (Q17.15)
    let f = f / 32768.0;
    (f * 10_0000.0).floor() / 10_0000.0
}

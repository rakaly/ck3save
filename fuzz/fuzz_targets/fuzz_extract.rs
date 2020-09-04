#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let _ = ck3save::Ck3Extractor::extract_header(data);
    let _ = ck3save::Ck3Extractor::extract_save(Cursor::new(data));
});

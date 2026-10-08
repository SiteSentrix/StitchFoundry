#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let _ = vectorcraft_affinity::inspect(bytes);
    let _ = vectorcraft_affinity::preview(bytes);
});

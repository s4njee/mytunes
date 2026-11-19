#![no_main]

use std::io::Cursor;
use std::path::Path;

use libfuzzer_sys::fuzz_target;
use mt_meta::read_metadata_from;

fuzz_target!(init: {
    // Match the application's unwind behavior so the reader can turn dependency
    // panics into typed errors. Uncaught panics still abort at the fuzz ABI.
    std::panic::set_hook(Box::new(|_| {}));
}, |data: &[u8]| {
    let _ = read_metadata_from(Cursor::new(data), Path::new("input.mp3"));
});

#![no_main]
use libfuzzer_sys::fuzz_target;

use std::collections::HashSet;

fn chunks_for(buf: &[u8]) -> HashSet<Vec<u8>> {
    let mut seen = HashSet::new();
    let c = std::io::Cursor::new(&buf);
    let mut chunker = libsnapcd::chunker::Chunker::new(c);

    while let Some(c) = chunker.next_chunk().unwrap() {
        seen.insert(c.buf().to_vec());
    }

    seen
}

fuzz_target!(|data: (Vec<u8>, usize, usize)| {
    let (mut buf, start, end) = data;

    if start >= end {
        return;
    }

    if end >= buf.len() {
        return;
    }

    if end - start > 255 {
        return;
    }

    let before = chunks_for(&buf);
    buf.splice(start..end, std::iter::empty());
    let after = chunks_for(&buf);

    let difference = after.difference(&before).count();

    // This isn't *guaranteed* to pass (maybe). The stricter <= 2 *does* fail sometimes.
    // If this fails, it's *maybe* fine? I need to do some reasoning about if I can put an upper
    // bound on the number of new bytes that can be forced to be stored for a N byte change.
    //
    // For normal rolling hashes, the upper bound for a "small" change (where small means it
    // doesn't consist of a whole chunk its in entirely being spanned) is 2: One for the chunk
    // itself, and maybe one for the later chunk. The resync should happen after that.
    //
    // I think it failing for us depends on max_size's value. Who knows.
    assert!(difference <= 2, "Difference was {}, expected <= 3", difference);
});

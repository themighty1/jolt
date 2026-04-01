#![cfg_attr(feature = "guest", no_std)]

#[jolt::provable(heap_size = 32768, max_trace_length = 65536)]
fn integer_check(input: &[u8]) -> (u32, [u8; 32]) {
    let hash = jolt_inlines_blake3::Blake3::digest(input);

    let s = match core::str::from_utf8(input) {
        Ok(s) => s,
        Err(_) => return (0, hash),
    };

    let trimmed = s.trim().as_bytes();
    if trimmed.is_empty() {
        return (0, hash);
    }

    let mut n: u64 = 0;
    let mut i = 0;
    while i < trimmed.len() {
        let b = trimmed[i];
        if !b.is_ascii_digit() {
            return (0, hash);
        }
        n = match n.checked_mul(10).and_then(|v| v.checked_add((b - b'0') as u64)) {
            Some(v) => v,
            None => return (0, hash),
        };
        i += 1;
    }

    if n > 700 { (1, hash) } else { (0, hash) }
}

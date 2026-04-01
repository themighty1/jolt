#![cfg_attr(feature = "guest", no_std)]

extern crate alloc;
use alloc::vec::Vec;

fn skip_ws(data: &[u8], mut i: usize) -> usize {
    while i < data.len() && matches!(data[i], b' ' | b'\n' | b'\r' | b'\t') {
        i += 1;
    }
    i
}

// --- Full JSON validator (recursive descent) ---

fn validate(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }
    let (pos, ok) = validate_value(data, skip_ws(data, 0));
    if !ok {
        return false;
    }
    skip_ws(data, pos) == data.len()
}

fn validate_value(data: &[u8], i: usize) -> (usize, bool) {
    if i >= data.len() {
        return (i, false);
    }
    match data[i] {
        b'"' => validate_string(data, i),
        b'{' => validate_object(data, i),
        b'[' => validate_array(data, i),
        b't' => validate_literal(data, i, b"true"),
        b'f' => validate_literal(data, i, b"false"),
        b'n' => validate_literal(data, i, b"null"),
        b'-' | b'0'..=b'9' => validate_number(data, i),
        _ => (i, false),
    }
}

fn validate_string(data: &[u8], mut i: usize) -> (usize, bool) {
    if i >= data.len() || data[i] != b'"' {
        return (i, false);
    }
    i += 1;
    while i < data.len() {
        match data[i] {
            b'\\' => {
                i += 1;
                if i >= data.len() {
                    return (i, false);
                }
                match data[i] {
                    b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => i += 1,
                    b'u' => {
                        if i + 4 >= data.len() {
                            return (i, false);
                        }
                        let mut k = 0;
                        while k < 4 {
                            if !data[i + 1 + k].is_ascii_hexdigit() {
                                return (i, false);
                            }
                            k += 1;
                        }
                        i += 5;
                    }
                    _ => return (i, false),
                }
            }
            b'"' => return (i + 1, true),
            b if b < 0x20 => return (i, false),
            _ => i += 1,
        }
    }
    (i, false)
}

fn validate_number(data: &[u8], mut i: usize) -> (usize, bool) {
    if i < data.len() && data[i] == b'-' {
        i += 1;
    }
    if i >= data.len() || !data[i].is_ascii_digit() {
        return (i, false);
    }
    if data[i] == b'0' {
        i += 1;
    } else {
        while i < data.len() && data[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i < data.len() && data[i] == b'.' {
        i += 1;
        if i >= data.len() || !data[i].is_ascii_digit() {
            return (i, false);
        }
        while i < data.len() && data[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i < data.len() && (data[i] == b'e' || data[i] == b'E') {
        i += 1;
        if i < data.len() && (data[i] == b'+' || data[i] == b'-') {
            i += 1;
        }
        if i >= data.len() || !data[i].is_ascii_digit() {
            return (i, false);
        }
        while i < data.len() && data[i].is_ascii_digit() {
            i += 1;
        }
    }
    (i, true)
}

fn validate_literal(data: &[u8], i: usize, expected: &[u8]) -> (usize, bool) {
    let end = i + expected.len();
    if end > data.len() {
        return (i, false);
    }
    if data[i..end] == *expected {
        (end, true)
    } else {
        (i, false)
    }
}

fn validate_object(data: &[u8], mut i: usize) -> (usize, bool) {
    i += 1; // skip '{'
    i = skip_ws(data, i);
    if i < data.len() && data[i] == b'}' {
        return (i + 1, true);
    }
    loop {
        i = skip_ws(data, i);
        let (next, ok) = validate_string(data, i);
        if !ok {
            return (next, false);
        }
        i = skip_ws(data, next);
        if i >= data.len() || data[i] != b':' {
            return (i, false);
        }
        i = skip_ws(data, i + 1);
        let (next, ok) = validate_value(data, i);
        if !ok {
            return (next, false);
        }
        i = skip_ws(data, next);
        if i >= data.len() {
            return (i, false);
        }
        if data[i] == b'}' {
            return (i + 1, true);
        }
        if data[i] != b',' {
            return (i, false);
        }
        i += 1;
    }
}

fn validate_array(data: &[u8], mut i: usize) -> (usize, bool) {
    i += 1; // skip '['
    i = skip_ws(data, i);
    if i < data.len() && data[i] == b']' {
        return (i + 1, true);
    }
    loop {
        i = skip_ws(data, i);
        let (next, ok) = validate_value(data, i);
        if !ok {
            return (next, false);
        }
        i = skip_ws(data, next);
        if i >= data.len() {
            return (i, false);
        }
        if data[i] == b']' {
            return (i + 1, true);
        }
        if data[i] != b',' {
            return (i, false);
        }
        i += 1;
    }
}

// --- Dot-path query (reuses skip functions from navigator) ---

fn skip_string_nav(data: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < data.len() {
        if data[i] == b'\\' {
            i += 2;
        } else if data[i] == b'"' {
            return i + 1;
        } else {
            i += 1;
        }
    }
    i
}

fn skip_value_nav(data: &[u8], i: usize) -> usize {
    let i = skip_ws(data, i);
    if i >= data.len() {
        return i;
    }
    match data[i] {
        b'"' => skip_string_nav(data, i),
        b'{' => skip_container(data, i, b'{', b'}'),
        b'[' => skip_container(data, i, b'[', b']'),
        _ => {
            let mut j = i;
            while j < data.len()
                && !matches!(data[j], b',' | b'}' | b']' | b' ' | b'\n' | b'\r' | b'\t')
            {
                j += 1;
            }
            j
        }
    }
}

fn skip_container(data: &[u8], mut i: usize, open: u8, close: u8) -> usize {
    let mut depth = 1;
    i += 1;
    while i < data.len() && depth > 0 {
        match data[i] {
            b'"' => i = skip_string_nav(data, i),
            b if b == open => {
                depth += 1;
                i += 1;
            }
            b if b == close => {
                depth -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    i
}

fn match_key(data: &[u8], i: usize, key: &[u8]) -> bool {
    if data[i] != b'"' {
        return false;
    }
    let key_start = i + 1;
    let key_end = key_start + key.len();
    if key_end >= data.len() || data[key_end] != b'"' {
        return false;
    }
    data[key_start..key_end] == *key
}

fn find_object_value(data: &[u8], pos: usize, key: &[u8]) -> usize {
    let mut i = skip_ws(data, pos + 1);
    while i < data.len() && data[i] != b'}' {
        if match_key(data, i, key) {
            i = skip_string_nav(data, i);
            i = skip_ws(data, i);
            return skip_ws(data, i + 1); // skip ':'
        }
        i = skip_string_nav(data, i);
        i = skip_ws(data, i);
        i = skip_ws(data, i + 1); // skip ':'
        i = skip_value_nav(data, i);
        i = skip_ws(data, i);
        if i < data.len() && data[i] == b',' {
            i = skip_ws(data, i + 1);
        }
    }
    panic!("key not found");
}

fn find_array_element(data: &[u8], pos: usize, idx: usize) -> usize {
    let mut i = skip_ws(data, pos + 1);
    let mut current = 0;
    while i < data.len() && data[i] != b']' {
        if current == idx {
            return skip_ws(data, i);
        }
        i = skip_value_nav(data, i);
        i = skip_ws(data, i);
        if i < data.len() && data[i] == b',' {
            i = skip_ws(data, i + 1);
        }
        current += 1;
    }
    panic!("index out of bounds");
}

fn extract_u64(data: &[u8], pos: usize) -> u64 {
    let mut i = skip_ws(data, pos);
    let mut n: u64 = 0;
    while i < data.len() && data[i].is_ascii_digit() {
        n = n * 10 + (data[i] - b'0') as u64;
        i += 1;
    }
    n
}

fn navigate(data: &[u8], path: &[u8]) -> u64 {
    let mut pos = skip_ws(data, 0);
    let mut seg_start = 0;

    while seg_start <= path.len() {
        let mut seg_end = seg_start;
        while seg_end < path.len() && path[seg_end] != b'.' {
            seg_end += 1;
        }
        if seg_start == seg_end && seg_start == path.len() {
            break;
        }
        let segment = &path[seg_start..seg_end];

        if data[pos] == b'{' {
            pos = find_object_value(data, pos, segment);
        } else if data[pos] == b'[' {
            let mut idx: usize = 0;
            let mut k = 0;
            while k < segment.len() {
                idx = idx * 10 + (segment[k] - b'0') as usize;
                k += 1;
            }
            pos = find_array_element(data, pos, idx);
        } else {
            panic!("unexpected json token");
        }

        seg_start = if seg_end < path.len() { seg_end + 1 } else { seg_end };
    }

    extract_u64(data, pos)
}

#[jolt::provable(
    max_input_size = 4096,
    max_untrusted_advice_size = 8192,
    stack_size = 65536,
    max_trace_length = 1048576
)]
fn json_query(query: &[u8], json_data: jolt::PrivateInput<Vec<u8>>) -> (u64, [u8; 32], [u8; 32]) {
    let json = &*json_data;

    let full_hash = jolt_inlines_blake2::Blake2b::digest(json);
    let mut hash_lo = [0u8; 32];
    let mut hash_hi = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        hash_lo[i] = full_hash[i];
        hash_hi[i] = full_hash[i + 32];
        i += 1;
    }

    assert!(validate(json), "invalid JSON");
    (navigate(json, query), hash_lo, hash_hi)
}

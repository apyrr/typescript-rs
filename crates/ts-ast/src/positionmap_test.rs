use std::fs;

use crate::*;

#[test]
fn position_map_ascii() {
    let text = "const x = 1;";
    let pm = compute_position_map(text);
    assert!(pm.is_ascii_only(), "expected ASCII-only");
    for i in 0..=text.len() as i32 {
        assert_eq!(pm.utf8_to_utf16(i), i, "UTF8ToUTF16({i})");
        assert_eq!(pm.utf16_to_utf8(i), i, "UTF16ToUTF8({i})");
    }
}

#[test]
fn position_map_two_byte() {
    // "cafe" with acute e: U+00E9 is 2 bytes UTF-8, 1 code unit UTF-16
    let text = "const caf\u{00e9} = 1;\nconst x = 2;";
    let pm = compute_position_map(text);
    assert!(!pm.is_ascii_only(), "expected non-ASCII");

    // Everything before e-acute (byte offset 9) should be identity
    for i in 0..10 {
        assert_eq!(pm.utf8_to_utf16(i), i, "before e-acute");
    }

    // e-acute starts at UTF-8 byte 9, UTF-16 offset 9: same
    assert_eq!(pm.utf8_to_utf16(9), 9, "at e-acute");

    // After e-acute, delta is 1
    // ' ' after cafe-acute: UTF-8 byte 11, UTF-16 offset 10
    assert_eq!(pm.utf8_to_utf16(11), 10, "after e-acute");

    // 'x' on second line: UTF-8 byte 23, UTF-16 offset 22
    let x_utf8 = text.rfind('x').unwrap() as i32;
    assert_eq!(pm.utf8_to_utf16(x_utf8), x_utf8 - 1, "at x");

    // Reverse: UTF-16 offset 22 should map to UTF-8 byte 23
    let x_utf16 = x_utf8 - 1;
    assert_eq!(pm.utf16_to_utf8(x_utf16), x_utf8, "reverse at x");
}

#[test]
fn position_map_four_byte() {
    // U+1F389 is 4 bytes UTF-8, 2 code units UTF-16
    let text = "const a = \"\u{1f389}\";\nconst b = 2;";
    let pm = compute_position_map(text);
    assert!(!pm.is_ascii_only(), "expected non-ASCII");

    // The emoji starts at byte 11 (after `const a = "`)
    // UTF-8: bytes 11-14 (4 bytes), UTF-16: units 11-12 (2 code units)
    // After the emoji: UTF-8 byte 15, UTF-16 offset 13. Delta = 2.

    // 'b' on second line
    let b_utf8 = text.rfind('b').unwrap() as i32;
    let b_utf16 = b_utf8 - 2; // delta of 2 from emoji
    assert_eq!(pm.utf8_to_utf16(b_utf8), b_utf16, "at b");
    assert_eq!(pm.utf16_to_utf8(b_utf16), b_utf8, "reverse at b");
}

#[test]
fn position_map_multiple_non_ascii() {
    // Mix of 2-byte and 4-byte characters
    // "a" with grave (U+00E0) = 2 bytes UTF-8, 1 code unit UTF-16 (delta +1)
    // U+1F389 = 4 bytes UTF-8, 2 code units UTF-16 (delta +2)
    let text = "\u{00e0}\u{1f389}x";
    let pm = compute_position_map(text);

    // a-grave: UTF-8 [0,2), UTF-16 [0,1)
    // emoji: UTF-8 [2,6), UTF-16 [1,3)
    // x: UTF-8 [6,7), UTF-16 [3,4)
    let tests = [(0, 0), (2, 1), (6, 3), (7, 4)];
    for (utf8, utf16) in tests {
        assert_eq!(pm.utf8_to_utf16(utf8), utf16, "UTF8ToUTF16({utf8})");
        assert_eq!(pm.utf16_to_utf8(utf16), utf8, "UTF16ToUTF8({utf16})");
    }
}

#[test]
fn position_map_roundtrip() {
    let text = "let caf\u{00e9} = \"\u{1f389}\"; // na\u{00ef}ve";
    let pm = compute_position_map(text);

    // Convert every valid UTF-16 position to UTF-8 and back
    let utf16_len = pm.utf8_to_utf16(text.len() as i32);
    for i in 0..=utf16_len {
        let utf8_pos = pm.utf16_to_utf8(i);
        let back = pm.utf8_to_utf16(utf8_pos);
        assert_eq!(back, i, "roundtrip UTF16->UTF8->UTF16");
    }
}

#[test]
fn benchmark_compute_position_map_ascii() {
    // ~10KB of ASCII TypeScript-like code
    let line = "const variable = someFunction(argument1, argument2);\n";
    let text = line.repeat(200);
    let _ = compute_position_map(&text);
}

#[test]
fn benchmark_compute_position_map_non_ascii() {
    // Mix of ASCII and non-ASCII (comments with unicode)
    let line = "const caf\u{00e9} = \"h\u{00e9}llo w\u{00f6}rld \u{1f389}\";\n";
    let text = line.repeat(200);
    let _ = compute_position_map(&text);
}

#[test]
fn benchmark_utf8_to_utf16_ascii() {
    let line = "const variable = someFunction(argument1, argument2);\n";
    let text = line.repeat(200);
    let pm = compute_position_map(&text);
    let positions = [0, 100, 500, 1000, 5000, text.len() as i32 - 1];
    for p in positions {
        let _ = pm.utf8_to_utf16(p);
    }
}

#[test]
fn benchmark_utf8_to_utf16_non_ascii() {
    let line = "const caf\u{00e9} = \"h\u{00e9}llo w\u{00f6}rld \u{1f389}\";\n";
    let text = line.repeat(200);
    let pm = compute_position_map(&text);
    let positions = [0, 100, 500, 1000, 5000, text.len() as i32 - 1];
    for p in positions {
        let _ = pm.utf8_to_utf16(p);
    }
}

#[test]
fn benchmark_utf16_to_utf8_non_ascii() {
    let line = "const caf\u{00e9} = \"h\u{00e9}llo w\u{00f6}rld \u{1f389}\";\n";
    let text = line.repeat(200);
    let pm = compute_position_map(&text);
    let utf16_len = pm.utf8_to_utf16(text.len() as i32);
    let positions = [0, 100, 500, 1000, 3000, utf16_len - 1];
    for p in positions {
        let _ = pm.utf16_to_utf8(p);
    }
}

#[test]
fn benchmark_compute_position_map_checker_ts() {
    let checker_ts = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/typescript/src/compiler/checker.ts"
    );
    let Ok(data) = fs::read_to_string(checker_ts) else {
        return;
    };
    let _ = compute_position_map(&data);
}

use std::io::Write;
use std::process::{Command, Stdio};

use serde::Deserialize;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_lsproto::DocumentUriExt;

use super::{
    Converters, LspLineMap, Script, compute_lsp_line_starts, file_name_to_document_uri,
    new_converters,
};

#[test]
fn test_document_uri_to_file_name() {
    let tests = [
        ("file:///path/to/file.ts", "/path/to/file.ts"),
        ("file://server/share/file.ts", "//server/share/file.ts"),
        (
            "file:///d%3A/work/tsgo932/lib/utils.ts",
            "d:/work/tsgo932/lib/utils.ts",
        ),
        (
            "file:///D%3A/work/tsgo932/lib/utils.ts",
            "d:/work/tsgo932/lib/utils.ts",
        ),
        (
            "file:///d%3A/work/tsgo932/app/%28test%29/comp/comp-test.tsx",
            "d:/work/tsgo932/app/(test)/comp/comp-test.tsx",
        ),
        ("file:///path/to/file.ts#section", "/path/to/file.ts"),
        ("file:///c:/test/me", "c:/test/me"),
        ("file://shares/files/c%23/p.cs", "//shares/files/c#/p.cs"),
        (
            "file:///c:/Source/Z%C3%BCrich%20or%20Zurich%20(%CB%88zj%CA%8A%C9%99r%C9%AAk,/Code/resources/app/plugins/c%23/plugin.json",
            "c:/Source/Zürich or Zurich (ˈzjʊərɪk,/Code/resources/app/plugins/c#/plugin.json",
        ),
        ("file:///c:/test %25/path", "c:/test %/path"),
        ("file:///_:/path", "/_:/path"),
        ("file:///users/me/c%23-projects/", "/users/me/c#-projects/"),
        (
            "file://localhost/c%24/GitDevelopment/express",
            "//localhost/c$/GitDevelopment/express",
        ),
        (
            "file:///c%3A/test%20with%20%2525/c%23code",
            "c:/test with %25/c#code",
        ),
        (
            "untitled:Untitled-1",
            "^/untitled/ts-nul-authority/Untitled-1",
        ),
        (
            "untitled:Untitled-1#fragment",
            "^/untitled/ts-nul-authority/Untitled-1#fragment",
        ),
        (
            "untitled:c:/Users/jrieken/Code/abc.txt",
            "^/untitled/ts-nul-authority/c:/Users/jrieken/Code/abc.txt",
        ),
        (
            "untitled:C:/Users/jrieken/Code/abc.txt",
            "^/untitled/ts-nul-authority/C:/Users/jrieken/Code/abc.txt",
        ),
        (
            "untitled://wsl%2Bubuntu/home/jabaile/work/TypeScript-go/newfile.ts",
            "^/untitled/wsl%2Bubuntu/home/jabaile/work/TypeScript-go/newfile.ts",
        ),
    ];

    for (uri, file_name) in tests {
        let uri: lsproto::DocumentUri = uri.to_string();
        assert_eq!(uri.file_name(), file_name, "{uri}");
    }
}

#[test]
fn test_file_name_to_document_uri() {
    let tests = [
        ("/path/to/file.ts", "file:///path/to/file.ts"),
        ("//server/share/file.ts", "file://server/share/file.ts"),
        (
            "d:/work/tsgo932/lib/utils.ts",
            "file:///d%3A/work/tsgo932/lib/utils.ts",
        ),
        (
            "d:/work/tsgo932/lib/utils.ts",
            "file:///d%3A/work/tsgo932/lib/utils.ts",
        ),
        (
            "d:/work/tsgo932/app/(test)/comp/comp-test.tsx",
            "file:///d%3A/work/tsgo932/app/%28test%29/comp/comp-test.tsx",
        ),
        ("/path/to/file.ts", "file:///path/to/file.ts"),
        ("c:/test/me", "file:///c%3A/test/me"),
        ("//shares/files/c#/p.cs", "file://shares/files/c%23/p.cs"),
        (
            "c:/Source/Zürich or Zurich (ˈzjʊərɪk,/Code/resources/app/plugins/c#/plugin.json",
            "file:///c%3A/Source/Z%C3%BCrich%20or%20Zurich%20%28%CB%88zj%CA%8A%C9%99r%C9%AAk%2C/Code/resources/app/plugins/c%23/plugin.json",
        ),
        ("c:/test %/path", "file:///c%3A/test%20%25/path"),
        ("/", "file:///"),
        ("/_:/path", "file:///_%3A/path"),
        ("/users/me/c#-projects/", "file:///users/me/c%23-projects/"),
        (
            "//localhost/c$/GitDevelopment/express",
            "file://localhost/c%24/GitDevelopment/express",
        ),
        (
            "c:/test with %25/c#code",
            "file:///c%3A/test%20with%20%2525/c%23code",
        ),
        (
            "^/untitled/ts-nul-authority/Untitled-1",
            "untitled:Untitled-1",
        ),
        (
            "^/untitled/ts-nul-authority/c:/Users/jrieken/Code/abc.txt",
            "untitled:c:/Users/jrieken/Code/abc.txt",
        ),
        (
            "^/untitled/ts-nul-authority///wsl%2Bubuntu/home/jabaile/work/TypeScript-go/newfile.ts",
            "untitled://wsl%2Bubuntu/home/jabaile/work/TypeScript-go/newfile.ts",
        ),
    ];

    for (file_name, uri) in tests {
        assert_eq!(file_name_to_document_uri(file_name), uri.to_string());
    }
}

struct TestScript {
    name: String,
    text: String,
}

impl Script for TestScript {
    fn file_name(&self) -> &str {
        &self.name
    }

    fn text(&self) -> &str {
        &self.text
    }
}

fn new_test_converters(text: &str) -> (Converters, TestScript) {
    let script = TestScript {
        name: "test.ts".to_string(),
        text: text.to_string(),
    };
    let line_map = compute_lsp_line_starts(text);
    let conv = new_converters(lsproto::PositionEncodingKind::UTF16, move |_| {
        line_map.clone()
    });
    (conv, script)
}

// Upstream TestConvertersInvalidUTF8 uses text := "a\x80b\ncd". Constructing an
// invalid `&str` in Rust would violate `str`'s validity invariant, and lossy
// decoding would change byte offsets, so this test is intentionally absent until
// lsconv has a byte-backed script-text abstraction.

// jsReferenceScript is a Node.js script that, given a list of UTF-8 byte buffers,
// computes the authoritative mapping between (line, character in UTF-16 code
// units) and UTF-8 byte offsets.
//
// To avoid any string round-tripping at the protocol boundary, the inputs are
// sent as raw bytes: the test writes a length-prefixed binary stream to stdin
// ([uint32 little-endian count][uint32 LE len][bytes]...[uint32 LE len][bytes]).
// Node reads the buffers and decodes each with TextDecoder('utf-8') -- which is
// essentially what tsserver / sys.ts does when reading file contents from disk
// (read as Buffer, decode as UTF-8 to a JS string with real UTF-16 semantics).
//
// For each input buffer, Node walks the underlying UTF-8 bytes (NOT the decoded
// string) to identify codepoint boundaries: every byte is the start of a
// codepoint unless it's a UTF-8 continuation byte (0b10xxxxxx). At each boundary
// it records the UTF-8 byte offset and the corresponding UTF-16 code unit offset
// (in the decoded JS string) and (line, char) using the LSP line-break rules
// (\n, \r, \r\n only).
//
// Output is JSON on stdout: [ [ { bytePos, line, char }, ... ], ... ]
const JS_REFERENCE_SCRIPT: &str = r#"
const inChunks = [];
process.stdin.on('data', c => inChunks.push(c));
process.stdin.on('end', () => {
  const buf = Buffer.concat(inChunks);
  let off = 0;
  const readU32 = () => { const v = buf.readUInt32LE(off); off += 4; return v; };
  const n = readU32();
  const buffers = [];
  for (let i = 0; i < n; i++) {
    const len = readU32();
    buffers.push(buf.subarray(off, off + len));
    off += len;
  }

  const decoder = new TextDecoder('utf-8', { fatal: true });
  const out = buffers.map(bytes => {
    // Decode the raw UTF-8 bytes to a JS string (this is what sys.ts does with file contents).
    const text = decoder.decode(bytes);

    // LSP line starts in the *decoded* JS string: \n, \r, \r\n only.
    const lineStartsJs = [0];
    for (let i = 0; i < text.length; i++) {
      const c = text.charCodeAt(i);
      if (c === 13) {
        if (i + 1 < text.length && text.charCodeAt(i + 1) === 10) i++;
        lineStartsJs.push(i + 1);
      } else if (c === 10) {
        lineStartsJs.push(i + 1);
      }
    }

    // Walk the original UTF-8 byte buffer to find codepoint boundaries. Inputs are
    // valid UTF-8, so we advance bytePos by the sequence length of each lead byte
    // and jsIdx by the corresponding UTF-16 code unit count (1 for BMP, 2 for
    // surrogate pair) of the codepoint at jsIdx in the decoded string.
    const boundaries = [{ bytePos: 0, jsIdx: 0 }];
    let bytePos = 0, jsIdx = 0;
    while (bytePos < bytes.length) {
      const seq = utf8SeqLen(bytes[bytePos]);
      const cp = text.codePointAt(jsIdx);
      bytePos += seq;
      jsIdx += cp > 0xFFFF ? 2 : 1;
      boundaries.push({ bytePos, jsIdx });
    }

    return boundaries.map(({ bytePos, jsIdx }) => {
      let lo = 0, hi = lineStartsJs.length - 1;
      while (lo < hi) {
        const mid = (lo + hi + 1) >> 1;
        if (lineStartsJs[mid] <= jsIdx) lo = mid;
        else hi = mid - 1;
      }
      return { bytePos, line: lo, char: jsIdx - lineStartsJs[lo] };
    });
  });

  process.stdout.write(JSON.stringify(out));
});

function utf8SeqLen(b) {
  if (b < 0x80) return 1;
  if ((b & 0xE0) === 0xC0) return 2;
  if ((b & 0xF0) === 0xE0) return 3;
  if ((b & 0xF8) === 0xF0) return 4;
  throw new Error('invalid UTF-8 lead byte 0x' + b.toString(16));
}
"#;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct JsTuple {
    #[serde(rename = "bytePos")]
    byte_pos: usize,
    line: u32,
    char: u32,
}

fn run_js_reference(texts: &[&str]) -> Option<Vec<Vec<JsTuple>>> {
    let mut child = Command::new("node")
        .arg("-e")
        .arg(JS_REFERENCE_SCRIPT)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    {
        let stdin = child.stdin.as_mut()?;
        // Build a length-prefixed binary stream of the raw UTF-8 bytes:
        // [uint32 LE count] then for each: [uint32 LE length][bytes].
        stdin.write_all(&(texts.len() as u32).to_le_bytes()).ok()?;
        for text in texts {
            stdin.write_all(&(text.len() as u32).to_le_bytes()).ok()?;
            stdin.write_all(text.as_bytes()).ok()?;
        }
    }

    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }

    serde_json::from_slice(&output.stdout).ok()
}

// TestConvertersAgainstJSReference cross-checks the Rust UTF-16 conversions
// against authoritative results computed by Node.js using real UTF-16 string
// semantics.
#[test]
fn test_converters_against_js_reference() {
    let cases = [
        ("empty", ""),
        ("ascii", "hello\nworld"),
        ("ascii_crlf", "hello\r\nworld\r\n!"),
        ("ascii_cr_only", "a\rb\rc"),
        ("trailing_newline", "abc\n"),
        ("bmp_em_dash", "ab\u{2014}cd\nef"),
        ("bmp_multi", "α\nβ\nγδε\nzz"),
        ("supplementary_emoji", "x\u{1F600}y\nz"),
        ("supplementary_at_lineend", "ab\u{1F600}\ncd\u{1F60A}"),
        ("supplementary_only", "\u{1F600}\u{1F601}\u{1F602}"),
        ("mixed", "α — \u{1F600}\r\nβ\nγ\r"),
        ("long_mixed_ws", "  \tαβ\n\t\u{1F600}  end\n"),
        ("zwj_emoji", "\u{1F468}\u{200D}\u{1F4BB}\nnext"),
        ("only_newlines", "\n\n\r\n\r"),
    ];

    let texts = cases.iter().map(|(_, text)| *text).collect::<Vec<_>>();
    let Some(refs) = run_js_reference(&texts) else {
        return;
    };

    assert_eq!(refs.len(), cases.len());

    for ((name, text), reference) in cases.iter().zip(refs.iter()) {
        let (conv, script) = new_test_converters(text);
        for tuple in reference {
            let expected = lsproto::Position {
                line: tuple.line,
                character: tuple.char,
            };

            let got_lc =
                conv.position_to_line_and_character(&script, tuple.byte_pos as core::TextPos);
            assert_eq!(
                got_lc, expected,
                "position_to_line_and_character({}) mismatch in {}",
                tuple.byte_pos, name
            );

            let got_pos = conv.line_and_character_to_position(&script, expected);
            assert_eq!(
                got_pos, tuple.byte_pos as core::TextPos,
                "line_and_character_to_position({}, {}) mismatch in {}",
                tuple.line, tuple.char, name
            );
        }
    }
}

#[allow(dead_code)]
fn _assert_lsp_line_map_type(_: &LspLineMap) {}

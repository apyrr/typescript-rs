use std::{fmt::Write, fs};

use ts_ast::{Kind, SourceFileParseOptions};
use ts_core as core;
use ts_parser as parser;
use ts_repo as repo;
use ts_testutil::baseline;

use super::*;

#[test]
fn encode_source_file() {
    super::testmain_test::test_main();

    let source_file = parse_source_file(
        SourceFileParseOptions {
            file_name: "/test.ts".to_string(),
            path: "/test.ts".to_string(),
            ..Default::default()
        },
        "import { bar } from \"bar\";\nexport function foo<T, U>(a: string, b: string): any {}\nfoo();",
        core::ScriptKind::TS,
    );

    let buf = super::encode_source_file(&source_file).unwrap();
    let str_ = format_encoded_source_file(&buf);
    baseline::run(
        "encodeSourceFile.txt",
        &str_,
        baseline::Options {
            subfolder: "api".to_string(),
            ..Default::default()
        },
    )
    .unwrap();
}

#[test]
fn encode_source_file_with_unicode_escapes() {
    super::testmain_test::test_main();

    let source_file = parse_source_file(
        SourceFileParseOptions {
            file_name: "/test.ts".to_string(),
            path: "/test.ts".to_string(),
            ..Default::default()
        },
        r#"let a = "😃"; let b = "\ud83d\ude03"; let c = "\udc00\ud83d\ude03"; let d = "\ud83d\ud83d\ude03""#,
        core::ScriptKind::TS,
    );

    let buf = super::encode_source_file(&source_file).unwrap();
    let str_ = format_encoded_source_file(&buf);
    baseline::run(
        "encodeSourceFileWithUnicodeEscapes.txt",
        &str_,
        baseline::Options {
            subfolder: "api".to_string(),
            ..Default::default()
        },
    )
    .unwrap();
}

fn benchmark_encode_source_file() {
    super::testmain_test::test_main();

    if repo::skip_if_no_type_script_submodule() {
        return;
    }
    let file_path = type_script_submodule_path().join("src/compiler/checker.ts");
    let file_content = fs::read_to_string(file_path).unwrap();
    let source_file = parse_source_file(
        SourceFileParseOptions {
            file_name: "/checker.ts".to_string(),
            path: "/checker.ts".to_string(),
            ..Default::default()
        },
        &file_content,
        core::ScriptKind::TS,
    );

    for _ in 0..1 {
        let _ = super::encode_source_file(&source_file).unwrap();
    }
}

fn read_u32(buf: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap())
}

fn format_encoded_source_file(encoded: &[u8]) -> String {
    let mut result = String::new();
    let offset_nodes = read_u32(encoded, HEADER_OFFSET_NODES) as usize;
    let offset_string_offsets = read_u32(encoded, HEADER_OFFSET_STRING_OFFSETS);
    let offset_strings = read_u32(encoded, HEADER_OFFSET_STRING_DATA);

    fn get_indent(encoded: &[u8], offset_nodes: usize, parent_index: u32) -> String {
        if parent_index == 0 {
            return String::new();
        }
        format!(
            "  {}",
            get_indent(
                encoded,
                offset_nodes,
                read_u32(
                    encoded,
                    offset_nodes + parent_index as usize * NODE_SIZE + NODE_OFFSET_PARENT,
                ),
            )
        )
    }

    let mut j = 1;
    let mut i = offset_nodes + NODE_SIZE;
    while i < encoded.len() {
        let kind = read_u32(encoded, i + NODE_OFFSET_KIND);
        let pos = read_u32(encoded, i + NODE_OFFSET_POS);
        let end = read_u32(encoded, i + NODE_OFFSET_END);
        let parent_index = read_u32(encoded, i + NODE_OFFSET_PARENT);
        result.push_str(&get_indent(encoded, offset_nodes, parent_index));
        if kind == SYNTAX_KIND_NODE_LIST {
            result.push_str("NodeList");
        } else {
            write!(&mut result, "{}", kind_from_u32(kind)).unwrap();
        }
        let data = read_u32(encoded, i + NODE_OFFSET_DATA);
        let data_type = data & NODE_DATA_TYPE_MASK;
        if kind_from_u32(kind) == Kind::Identifier || data_type == NODE_DATA_TYPE_STRING {
            let string_index = data & NODE_DATA_STRING_INDEX_MASK;
            let str_start = read_u32(encoded, (offset_string_offsets + string_index * 4) as usize);
            let str_end = read_u32(
                encoded,
                (offset_string_offsets + string_index * 4) as usize + 4,
            );
            let str_ = std::str::from_utf8(
                &encoded
                    [(offset_strings + str_start) as usize..(offset_strings + str_end) as usize],
            )
            .unwrap();
            write!(&mut result, " \"{}\"", str_).unwrap();
        }
        writeln!(
            &mut result,
            " [{}, {}), i={}, next={}",
            pos,
            end,
            j,
            encoded[i + NODE_OFFSET_NEXT]
        )
        .unwrap();
        j += 1;
        i += NODE_SIZE;
    }
    result
}

fn parse_source_file(
    opts: SourceFileParseOptions,
    text: &str,
    script_kind: core::ScriptKind,
) -> ts_ast::SourceFile {
    parser::parse_source_file(opts, text.to_owned(), script_kind)
}

fn type_script_submodule_path() -> std::path::PathBuf {
    repo::type_script_submodule_path().to_path_buf()
}

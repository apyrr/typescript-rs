use super::*;

fn generator() -> Generator {
    new_generator(
        "main.js".to_string(),
        "/".to_string(),
        "/".to_string(),
        ts_tspath::ComparePathsOptions::default(),
    )
}

fn assert_error<T: std::fmt::Debug>(actual: Result<T, String>, expected: &str) {
    assert_eq!(actual.unwrap_err(), expected);
}

#[test]
fn test_source_map_generator_empty() {
    let mut generator = generator();
    let source_map = generator.raw_source_map();
    assert_eq!(
        source_map,
        RawSourceMap {
            version: 3,
            file: "main.js".to_string(),
            source_root: "/".to_string(),
            sources: vec![],
            names: vec![],
            mappings: String::new(),
            sources_content: vec![],
        }
    );
}

#[test]
fn test_source_map_generator_empty_serialized() {
    let mut generator = generator();
    let actual = generator.string();
    let expected =
        r#"{"version":3,"file":"main.js","sourceRoot":"/","sources":[],"names":[],"mappings":""}"#;
    assert_eq!(actual, expected);
}

#[test]
fn test_source_map_generator_add_source() {
    let mut generator = generator();
    let source_index = generator.add_source("/main.ts".to_string());
    let source_map = generator.raw_source_map();
    assert_eq!(source_index, 0);
    assert_eq!(
        source_map,
        RawSourceMap {
            version: 3,
            file: "main.js".to_string(),
            source_root: "/".to_string(),
            sources: vec!["main.ts".to_string()],
            names: vec![],
            mappings: String::new(),
            sources_content: vec![],
        }
    );
}

#[test]
fn test_source_map_generator_set_source_content() {
    let mut generator = generator();
    let source_index = generator.add_source("/main.ts".to_string());
    let source_content = "foo".to_string();
    generator
        .set_source_content(source_index, source_content.clone())
        .unwrap();
    let source_map = generator.raw_source_map();
    assert_eq!(source_index, 0);
    assert_eq!(
        source_map,
        RawSourceMap {
            version: 3,
            file: "main.js".to_string(),
            source_root: "/".to_string(),
            sources: vec!["main.ts".to_string()],
            names: vec![],
            mappings: String::new(),
            sources_content: vec![Some(source_content)],
        }
    );
}

#[test]
fn test_source_map_generator_set_source_content_for_second_source_only() {
    let mut generator = generator();
    generator.add_source("/skipped.ts".to_string());
    let source_index = generator.add_source("/main.ts".to_string());
    let source_content = "foo".to_string();
    generator
        .set_source_content(source_index, source_content.clone())
        .unwrap();
    let source_map = generator.raw_source_map();
    assert_eq!(source_index, 1);
    assert_eq!(
        source_map,
        RawSourceMap {
            version: 3,
            file: "main.js".to_string(),
            source_root: "/".to_string(),
            sources: vec!["skipped.ts".to_string(), "main.ts".to_string()],
            names: vec![],
            mappings: String::new(),
            sources_content: vec![None, Some(source_content)],
        }
    );
}

#[test]
fn test_source_map_generator_set_source_content_source_index_out_of_range() {
    let mut generator = generator();
    assert_error(
        generator.set_source_content(-1, String::new()),
        "sourceIndex is out of range",
    );
    assert_error(
        generator.set_source_content(0, String::new()),
        "sourceIndex is out of range",
    );
}

#[test]
fn test_source_map_generator_set_source_content_for_second_source_only_serialized() {
    let mut generator = generator();
    generator.add_source("/skipped.ts".to_string());
    let source_index = generator.add_source("/main.ts".to_string());
    let source_content = "foo".to_string();
    generator
        .set_source_content(source_index, source_content)
        .unwrap();
    let actual = generator.string();
    let expected = r#"{"version":3,"file":"main.js","sourceRoot":"/","sources":["skipped.ts","main.ts"],"names":[],"mappings":"","sourcesContent":[null,"foo"]}"#;
    assert_eq!(actual, expected);
}

#[test]
fn test_source_map_generator_add_name() {
    let mut generator = generator();
    let name_index = generator.add_name("foo".to_string());
    let source_map = generator.raw_source_map();
    assert_eq!(name_index, 0);
    assert_eq!(
        source_map,
        RawSourceMap {
            version: 3,
            file: "main.js".to_string(),
            source_root: "/".to_string(),
            sources: vec![],
            names: vec!["foo".to_string()],
            mappings: String::new(),
            sources_content: vec![],
        }
    );
}

#[test]
fn test_source_map_generator_add_generated_mapping() {
    let mut generator = generator();
    generator.add_generated_mapping(0, 0).unwrap();
    let source_map = generator.raw_source_map();
    assert_eq!(
        source_map,
        RawSourceMap {
            version: 3,
            file: "main.js".to_string(),
            source_root: "/".to_string(),
            sources: vec![],
            names: vec![],
            mappings: "A".to_string(),
            sources_content: vec![],
        }
    );
}

#[test]
fn test_source_map_generator_add_generated_mapping_on_second_line_only() {
    let mut generator = generator();
    generator.add_generated_mapping(1, 0).unwrap();
    let source_map = generator.raw_source_map();
    assert_eq!(
        source_map,
        RawSourceMap {
            version: 3,
            file: "main.js".to_string(),
            source_root: "/".to_string(),
            sources: vec![],
            names: vec![],
            mappings: ";A".to_string(),
            sources_content: vec![],
        }
    );
}

#[test]
fn test_source_map_generator_add_source_mapping() {
    let mut generator = generator();
    let source_index = generator.add_source("/main.ts".to_string());
    generator
        .add_source_mapping(0, 0, source_index, 0, 0)
        .unwrap();
    let source_map = generator.raw_source_map();
    assert_eq!(
        source_map,
        RawSourceMap {
            version: 3,
            file: "main.js".to_string(),
            source_root: "/".to_string(),
            sources: vec!["main.ts".to_string()],
            names: vec![],
            mappings: "AAAA".to_string(),
            sources_content: vec![],
        }
    );
}

#[test]
fn test_source_map_generator_add_source_mapping_next_generated_character() {
    let mut generator = generator();
    let source_index = generator.add_source("/main.ts".to_string());
    generator
        .add_source_mapping(0, 0, source_index, 0, 0)
        .unwrap();
    generator
        .add_source_mapping(0, 1, source_index, 0, 0)
        .unwrap();
    let source_map = generator.raw_source_map();
    assert_eq!(
        source_map,
        RawSourceMap {
            version: 3,
            file: "main.js".to_string(),
            source_root: "/".to_string(),
            sources: vec!["main.ts".to_string()],
            names: vec![],
            mappings: "AAAA,CAAA".to_string(),
            sources_content: vec![],
        }
    );
}

#[test]
fn test_source_map_generator_add_source_mapping_next_generated_and_source_character() {
    let mut generator = generator();
    let source_index = generator.add_source("/main.ts".to_string());
    generator
        .add_source_mapping(0, 0, source_index, 0, 0)
        .unwrap();
    generator
        .add_source_mapping(0, 1, source_index, 0, 1)
        .unwrap();
    let source_map = generator.raw_source_map();
    assert_eq!(
        source_map,
        RawSourceMap {
            version: 3,
            file: "main.js".to_string(),
            source_root: "/".to_string(),
            sources: vec!["main.ts".to_string()],
            names: vec![],
            mappings: "AAAA,CAAC".to_string(),
            sources_content: vec![],
        }
    );
}

#[test]
fn test_source_map_generator_add_source_mapping_next_generated_line() {
    let mut generator = generator();
    let source_index = generator.add_source("/main.ts".to_string());
    generator
        .add_source_mapping(0, 0, source_index, 0, 0)
        .unwrap();
    generator
        .add_source_mapping(1, 0, source_index, 0, 0)
        .unwrap();
    let source_map = generator.raw_source_map();
    assert_eq!(
        source_map,
        RawSourceMap {
            version: 3,
            file: "main.js".to_string(),
            source_root: "/".to_string(),
            sources: vec!["main.ts".to_string()],
            names: vec![],
            mappings: "AAAA;AAAA".to_string(),
            sources_content: vec![],
        }
    );
}

#[test]
fn test_source_map_generator_add_source_mapping_previous_source_character() {
    let mut generator = generator();
    let source_index = generator.add_source("/main.ts".to_string());
    generator
        .add_source_mapping(0, 0, source_index, 0, 1)
        .unwrap();
    generator
        .add_source_mapping(0, 1, source_index, 0, 0)
        .unwrap();
    let source_map = generator.raw_source_map();
    assert_eq!(
        source_map,
        RawSourceMap {
            version: 3,
            file: "main.js".to_string(),
            source_root: "/".to_string(),
            sources: vec!["main.ts".to_string()],
            names: vec![],
            mappings: "AAAC,CAAD".to_string(),
            sources_content: vec![],
        }
    );
}

#[test]
fn test_source_map_generator_add_named_source_mapping() {
    let mut generator = generator();
    let source_index = generator.add_source("/main.ts".to_string());
    let name_index = generator.add_name("foo".to_string());
    generator
        .add_named_source_mapping(0, 0, source_index, 0, 0, name_index)
        .unwrap();
    let source_map = generator.raw_source_map();
    assert_eq!(
        source_map,
        RawSourceMap {
            version: 3,
            file: "main.js".to_string(),
            source_root: "/".to_string(),
            sources: vec!["main.ts".to_string()],
            names: vec!["foo".to_string()],
            mappings: "AAAAA".to_string(),
            sources_content: vec![],
        }
    );
}

#[test]
fn test_source_map_generator_add_named_source_mapping_with_previous_name() {
    let mut generator = generator();
    let source_index = generator.add_source("/main.ts".to_string());
    let name_index1 = generator.add_name("foo".to_string());
    let name_index2 = generator.add_name("bar".to_string());
    generator
        .add_named_source_mapping(0, 0, source_index, 0, 0, name_index2)
        .unwrap();
    generator
        .add_named_source_mapping(0, 1, source_index, 0, 0, name_index1)
        .unwrap();
    let source_map = generator.raw_source_map();
    assert_eq!(
        source_map,
        RawSourceMap {
            version: 3,
            file: "main.js".to_string(),
            source_root: "/".to_string(),
            sources: vec!["main.ts".to_string()],
            names: vec!["foo".to_string(), "bar".to_string()],
            mappings: "AAAAC,CAAAD".to_string(),
            sources_content: vec![],
        }
    );
}

#[test]
fn test_source_map_generator_add_generated_mapping_generated_line_cannot_backtrack() {
    let mut generator = generator();
    generator.add_generated_mapping(1, 0).unwrap();
    assert_error(
        generator.add_generated_mapping(0, 0),
        "generatedLine cannot backtrack",
    );
}

#[test]
fn test_source_map_generator_add_generated_mapping_generated_character_cannot_be_negative() {
    let mut generator = generator();
    generator.add_generated_mapping(0, 0).unwrap();
    assert_error(
        generator.add_generated_mapping(0, -1),
        "generatedCharacter cannot be negative",
    );
}

#[test]
fn test_source_map_generator_add_source_mapping_generated_line_cannot_backtrack() {
    let mut generator = generator();
    let source_index = generator.add_source("/main.ts".to_string());
    generator
        .add_source_mapping(1, 0, source_index, 0, 0)
        .unwrap();
    assert_error(
        generator.add_source_mapping(0, 0, source_index, 0, 0),
        "generatedLine cannot backtrack",
    );
}

#[test]
fn test_source_map_generator_add_source_mapping_generated_character_cannot_be_negative() {
    let mut generator = generator();
    let source_index = generator.add_source("/main.ts".to_string());
    generator
        .add_source_mapping(0, 0, source_index, 0, 0)
        .unwrap();
    assert_error(
        generator.add_source_mapping(0, -1, source_index, 0, 0),
        "generatedCharacter cannot be negative",
    );
}

#[test]
fn test_source_map_generator_add_source_mapping_source_index_is_out_of_range() {
    let mut generator = generator();
    assert_error(
        generator.add_source_mapping(0, 0, -1, 0, 0),
        "sourceIndex is out of range",
    );
    assert_error(
        generator.add_source_mapping(0, 0, 0, 0, 0),
        "sourceIndex is out of range",
    );
}

#[test]
fn test_source_map_generator_add_source_mapping_source_line_cannot_be_negative() {
    let mut generator = generator();
    let source_index = generator.add_source("/main.ts".to_string());
    assert_error(
        generator.add_source_mapping(0, 0, source_index, -1, 0),
        "sourceLine cannot be negative",
    );
}

#[test]
fn test_source_map_generator_add_source_mapping_source_character_cannot_be_negative() {
    let mut generator = generator();
    let source_index = generator.add_source("/main.ts".to_string());
    assert_error(
        generator.add_source_mapping(0, 0, source_index, 0, -1),
        "sourceCharacter cannot be negative",
    );
}

#[test]
fn test_source_map_generator_add_named_source_mapping_generated_line_cannot_backtrack() {
    let mut generator = generator();
    let source_index = generator.add_source("/main.ts".to_string());
    let name_index = generator.add_name("foo".to_string());
    generator
        .add_named_source_mapping(1, 0, source_index, 0, 0, name_index)
        .unwrap();
    assert_error(
        generator.add_named_source_mapping(0, 0, source_index, 0, 0, name_index),
        "generatedLine cannot backtrack",
    );
}

#[test]
fn test_source_map_generator_add_named_source_mapping_generated_character_cannot_be_negative() {
    let mut generator = generator();
    let source_index = generator.add_source("/main.ts".to_string());
    let name_index = generator.add_name("foo".to_string());
    generator
        .add_named_source_mapping(0, 0, source_index, 0, 0, name_index)
        .unwrap();
    assert_error(
        generator.add_named_source_mapping(0, -1, source_index, 0, 0, name_index),
        "generatedCharacter cannot be negative",
    );
}

#[test]
fn test_source_map_generator_add_named_source_mapping_source_index_is_out_of_range() {
    let mut generator = generator();
    let name_index = generator.add_name("foo".to_string());
    assert_error(
        generator.add_named_source_mapping(0, 0, -1, 0, 0, name_index),
        "sourceIndex is out of range",
    );
    assert_error(
        generator.add_named_source_mapping(0, 0, 0, 0, 0, name_index),
        "sourceIndex is out of range",
    );
}

#[test]
fn test_source_map_generator_add_named_source_mapping_source_line_cannot_be_negative() {
    let mut generator = generator();
    let name_index = generator.add_name("foo".to_string());
    let source_index = generator.add_source("/main.ts".to_string());
    assert_error(
        generator.add_named_source_mapping(0, 0, source_index, -1, 0, name_index),
        "sourceLine cannot be negative",
    );
}

#[test]
fn test_source_map_generator_add_named_source_mapping_source_character_cannot_be_negative() {
    let mut generator = generator();
    let name_index = generator.add_name("foo".to_string());
    let source_index = generator.add_source("/main.ts".to_string());
    assert_error(
        generator.add_named_source_mapping(0, 0, source_index, 0, -1, name_index),
        "sourceCharacter cannot be negative",
    );
}

#[test]
fn test_source_map_generator_add_named_source_mapping_name_index_is_out_of_range() {
    let mut generator = generator();
    let source_index = generator.add_source("/main.ts".to_string());
    assert_error(
        generator.add_named_source_mapping(0, 0, source_index, 0, 0, -1),
        "nameIndex is out of range",
    );
    assert_error(
        generator.add_named_source_mapping(0, 0, source_index, 0, 0, 0),
        "nameIndex is out of range",
    );
}

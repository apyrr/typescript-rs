use crate::{FourslashTest, RequestInfo, TestingT, send_request};
use ts_ls as lsconv;
use ts_lsproto as lsproto;

pub struct SemanticToken {
    pub type_: String,
    pub text: String,
}

impl FourslashTest {
    pub fn verify_semantic_tokens(&mut self, _t: &mut TestingT, expected: &[SemanticToken]) {
        let params = lsproto::SemanticTokensParams {
            work_done_token: None,
            partial_result_token: None,
            text_document: lsproto::TextDocumentIdentifier {
                uri: lsconv::file_name_to_document_uri(&self.active_filename),
            },
        };

        let result: lsproto::SemanticTokensResponse = send_request(
            _t,
            self,
            RequestInfo {
                method: lsproto::MethodTextDocumentSemanticTokensFull.to_string(),
                send: |f, params| {
                    f.send_lsp_request(lsproto::MethodTextDocumentSemanticTokensFull, params)
                },
                validate: |_result| true,
            },
            params,
        );

        if result.semantic_tokens.is_none() {
            if expected.is_empty() {
                return;
            }
            panic!("Expected semantic tokens but got nil");
        }
        let semantic_tokens = result.semantic_tokens.unwrap();

        // Decode the semantic tokens using token types/modifiers from the test configuration
        let actual = decode_semantic_tokens(
            self,
            &semantic_tokens.data,
            &self.semantic_token_types,
            &self.semantic_token_modifiers,
        );

        // Compare with expected
        if actual.len() != expected.len() {
            panic!(
                "Expected {} semantic tokens, got {}\n\nExpected:\n{}\n\nActual:\n{}",
                expected.len(),
                actual.len(),
                format_semantic_tokens(expected),
                format_semantic_tokens(&actual)
            );
        }

        for (i, (exp, act)) in expected.iter().zip(actual.iter()).enumerate() {
            if exp.type_ != act.type_ || exp.text != act.text {
                panic!(
                    "Token {} mismatch:\n  Expected: {{Type: {:?}, Text: {:?}}}\n  Actual:   {{Type: {:?}, Text: {:?}}}",
                    i, exp.type_, exp.text, act.type_, act.text
                );
            }
        }
    }
}

pub fn decode_semantic_tokens(
    f: &FourslashTest,
    data: &[u32],
    token_types: &[String],
    token_modifiers: &[String],
) -> Vec<SemanticToken> {
    if data.len() % 5 != 0 {
        panic!("Invalid semantic tokens data length: {}", data.len());
    }

    let script_info = f
        .script_infos
        .get(&f.active_filename)
        .unwrap_or_else(|| panic!("Script info for '{}' not found", f.active_filename));

    let mut tokens = Vec::new();
    let mut prev_line = 0_u32;
    let mut prev_char = 0_u32;

    let mut i = 0;
    while i < data.len() {
        let delta_line = data[i];
        let delta_char = data[i + 1];
        let length = data[i + 2];
        let token_type_idx = data[i + 3];
        let token_modifier_mask = data[i + 4];

        // Calculate absolute position
        let line = prev_line + delta_line;
        let char_ = if delta_line == 0 {
            prev_char + delta_char
        } else {
            delta_char
        };

        // Get token type
        if token_type_idx as usize >= token_types.len() {
            panic!("Token type index out of range: {}", token_type_idx);
        }
        let token_type = &token_types[token_type_idx as usize];

        // Get modifiers
        let mut modifiers = Vec::new();
        for (i, modifier) in token_modifiers.iter().enumerate() {
            if token_modifier_mask & (1 << i) != 0 {
                modifiers.push(modifier.clone());
            }
        }

        // Build full type string (type.modifier1.modifier2)
        let mut type_str = token_type.clone();
        if !modifiers.is_empty() {
            type_str.push('.');
            type_str.push_str(&modifiers.join("."));
        }

        // Get the text
        let start_pos = lsproto::Position {
            line,
            character: char_,
        };
        let end_pos = lsproto::Position {
            line,
            character: char_ + length,
        };
        let start_offset = f
            .converters
            .line_and_character_to_position(script_info, start_pos);
        let end_offset = f
            .converters
            .line_and_character_to_position(script_info, end_pos);
        let text = script_info.content[start_offset..end_offset].to_string();

        tokens.push(SemanticToken {
            type_: type_str,
            text,
        });

        prev_line = line;
        prev_char = char_;
        i += 5;
    }

    tokens
}

pub fn format_semantic_tokens(tokens: &[SemanticToken]) -> String {
    let mut lines = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        lines.push(format!(
            "  [{}] {{Type: {:?}, Text: {:?}}}",
            i, token.type_, token.text
        ));
    }
    lines.join("\n")
}

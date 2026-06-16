use ts_ast as ast;
use ts_astnav as astnav;
use ts_core as core;
use ts_debug as debug;
use ts_lsproto as lsproto;
use ts_scanner as scanner;

use crate::LanguageService;

// allow the client to match more than valid tag names. This allows linked editing when typing is in progress or tag name is incomplete
pub const JSX_TAG_WORD_PATTERN: &str = "[a-zA-Z0-9:\\-\\._$]*";

impl LanguageService<'_> {
    pub fn provide_linked_editing_range(
        &self,
        _ctx: &core::Context,
        params: &lsproto::LinkedEditingRangeParams,
    ) -> Result<lsproto::LinkedEditingRangeResponse, core::Error> {
        let (_, source_file) = self.get_program_and_file(
            params
                .text_document_position_params
                .text_document
                .uri
                .to_string(),
        );
        let position = self.converters.line_and_character_to_position(
            source_file,
            lsproto::Position {
                line: params.text_document_position_params.position.line,
                character: params.text_document_position_params.position.character,
            },
        );
        let token = astnav::find_preceding_token(source_file, position as i32);
        let store = source_file.store();

        if token
            .and_then(|token| store.parent(token))
            .is_none_or(|parent| store.kind(parent) == ast::Kind::SourceFile)
        {
            return Ok(lsproto::LinkedEditingRangeResponse::default());
        }
        let token = token.unwrap();

        let token_parent = store.parent(token).unwrap();
        let token_grandparent = store.parent(token_parent).unwrap();
        if ast::is_jsx_fragment(store, token_grandparent) {
            let open_fragment = store.opening_fragment(token_grandparent).unwrap();
            let close_fragment = store.closing_fragment(token_grandparent).unwrap();
            if store
                .flags(open_fragment)
                .intersects(ast::NodeFlags::ThisNodeOrAnySubNodesHasError)
                || store
                    .flags(close_fragment)
                    .intersects(ast::NodeFlags::ThisNodeOrAnySubNodesHasError)
            {
                return Ok(lsproto::LinkedEditingRangeResponse::default());
            }

            let open_pos = astnav::get_start_of_node(open_fragment, source_file) + "<".len() as i32;
            let close_pos =
                astnav::get_start_of_node(close_fragment, source_file) + "</".len() as i32;

            // only allows linked editing right after opening bracket: <| ></| >
            if position as i32 != open_pos && position as i32 != close_pos {
                return Ok(lsproto::LinkedEditingRangeResponse::default());
            }

            let open_line_char = self
                .converters
                .position_to_line_and_character(source_file, open_pos);
            let close_line_char = self
                .converters
                .position_to_line_and_character(source_file, close_pos);
            return Ok(lsproto::LinkedEditingRangeResponse {
                linked_editing_ranges: Some(lsproto::LinkedEditingRanges {
                    ranges: vec![
                        to_lsp_range(open_line_char, open_line_char), // only return start position for opening tag since the length of a fragment is always 3 and it is unlikely user will type in the middle of a fragment tag
                        to_lsp_range(close_line_char, close_line_char),
                    ],
                    word_pattern: Some(JSX_TAG_WORD_PATTERN.to_string()),
                }),
            });
        }

        // determines if the cursor is in an element tag
        let tag = ast::find_ancestor(store, Some(token_parent), |store, node| {
            ast::is_jsx_opening_element(store, node) || ast::is_jsx_closing_element(store, node)
        });
        let Some(tag) = tag else {
            return Ok(lsproto::LinkedEditingRangeResponse::default());
        };
        debug::assert(
            ast::is_jsx_opening_element(store, tag) || ast::is_jsx_closing_element(store, tag),
            Some("tag should be opening or closing element".to_string()),
        );

        let tag_parent = store.parent(tag).unwrap();
        let open_tag = store.opening_element(tag_parent).unwrap();
        let close_tag = store.closing_element(tag_parent).unwrap();

        let open_tag_name_start =
            astnav::get_start_of_node(store.tag_name(open_tag).unwrap(), source_file);
        let open_tag_name_end = store.loc(store.tag_name(open_tag).unwrap()).end();
        let close_tag_name_start =
            astnav::get_start_of_node(store.tag_name(close_tag).unwrap(), source_file);
        let close_tag_name_end = store.loc(store.tag_name(close_tag).unwrap()).end();
        // do not return linked cursors if tags are not well-formed
        if open_tag_name_start == astnav::get_start_of_node(open_tag, source_file)
            || close_tag_name_start == astnav::get_start_of_node(close_tag, source_file)
            || open_tag_name_end == store.loc(open_tag).end()
            || close_tag_name_end == store.loc(close_tag).end()
        {
            return Ok(lsproto::LinkedEditingRangeResponse::default());
        }
        // only return linked cursors if the cursor is within a tag name
        let position = position as i32;
        if !((open_tag_name_start <= position && position <= open_tag_name_end)
            || (close_tag_name_start <= position && position <= close_tag_name_end))
        {
            return Ok(lsproto::LinkedEditingRangeResponse::default());
        }

        // only return linked cursors if text in both tags is identical
        let opening_tag_text =
            scanner::get_text_of_node(source_file, &store.tag_name(open_tag).unwrap());
        if opening_tag_text
            != scanner::get_text_of_node(source_file, &store.tag_name(close_tag).unwrap())
        {
            return Ok(lsproto::LinkedEditingRangeResponse::default());
        }

        Ok(lsproto::LinkedEditingRangeResponse {
            linked_editing_ranges: Some(lsproto::LinkedEditingRanges {
                ranges: vec![
                    to_lsp_range(
                        self.converters
                            .position_to_line_and_character(source_file, open_tag_name_start),
                        self.converters
                            .position_to_line_and_character(source_file, open_tag_name_end),
                    ),
                    to_lsp_range(
                        self.converters
                            .position_to_line_and_character(source_file, close_tag_name_start),
                        self.converters
                            .position_to_line_and_character(source_file, close_tag_name_end),
                    ),
                ],
                word_pattern: Some(JSX_TAG_WORD_PATTERN.to_string()),
            }),
        })
    }
}

fn to_lsp_range(start: lsproto::Position, end: lsproto::Position) -> lsp_types_full::Range {
    lsp_types_full::Range {
        start: lsp_types_full::Position {
            line: start.line,
            character: start.character,
        },
        end: lsp_types_full::Position {
            line: end.line,
            character: end.character,
        },
    }
}

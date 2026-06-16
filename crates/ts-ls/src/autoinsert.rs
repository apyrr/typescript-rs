use ts_ast as ast;
use ts_astnav as astnav;
use ts_lsproto as lsproto;

use crate::LanguageService;

impl LanguageService<'_> {
    pub fn provide_on_auto_insert(
        &self,
        params: &lsproto::VsOnAutoInsertParams,
    ) -> Result<lsproto::VsOnAutoInsertResponse, ()> {
        if params.vs_ch != ">" {
            return Ok(lsproto::VsOnAutoInsertResponse::default());
        }

        let document_uri = params.vs_text_document.uri.to_string();
        let vs_position = lsproto::Position {
            line: params.vs_position.line,
            character: params.vs_position.character,
        };
        let (_, source_file) = self.get_program_and_file(document_uri);
        let position = self
            .converters
            .line_and_character_to_position(source_file, vs_position);

        let token = astnav::find_preceding_token(source_file, position as i32);
        let Some(token) = token else {
            return Ok(lsproto::VsOnAutoInsertResponse::default());
        };

        let mut closing_text = String::new();
        let store = source_file.store();
        let token_parent = store.parent(token);
        let element;
        if store.kind(token) == ast::Kind::GreaterThanToken
            && token_parent
                .as_ref()
                .is_some_and(|parent| ast::is_jsx_opening_element(store, *parent))
        {
            element = token_parent
                .as_ref()
                .and_then(|parent| store.parent(*parent));
        } else if ast::is_jsx_text(store, token)
            && token_parent
                .as_ref()
                .is_some_and(|parent| ast::is_jsx_element(store, *parent))
        {
            element = token_parent;
        } else {
            element = None;
        }

        if let Some(element) = element {
            if is_unclosed_tag(store, &element) {
                let opening_element = store.opening_element(element).unwrap();
                let tag_name_node = store.tag_name(opening_element).unwrap();
                // Slight divergence from Strada - we don't use the verbatim text from the opening tag.
                closing_text = format!(
                    "</{}>",
                    ast::entity_name_to_string(store, &tag_name_node, None)
                );
            }
        } else {
            let token_parent = store.parent(token);
            let fragment;
            if store.kind(token) == ast::Kind::GreaterThanToken
                && token_parent
                    .as_ref()
                    .is_some_and(|parent| ast::is_jsx_opening_fragment(store, *parent))
            {
                fragment = token_parent
                    .as_ref()
                    .and_then(|parent| store.parent(*parent));
            } else if ast::is_jsx_text(store, token)
                && token_parent
                    .as_ref()
                    .is_some_and(|parent| ast::is_jsx_fragment(store, *parent))
            {
                fragment = token_parent;
            } else {
                fragment = None;
            }

            if let Some(fragment) = fragment {
                if is_unclosed_fragment(store, &fragment) {
                    closing_text = "</>".to_string();
                }
            }
        }

        if closing_text.is_empty() {
            return Ok(lsproto::VsOnAutoInsertResponse::default());
        }

        Ok(lsproto::VsOnAutoInsertResponse {
            vs_on_auto_insert_response_item: Some(lsproto::VsOnAutoInsertResponseItem {
                vs_text_edit_format: lsproto::InsertTextFormat::Snippet,
                vs_text_edit: lsproto::TextEdit {
                    range: lsproto::Range {
                        start: vs_position,
                        end: vs_position,
                    },
                    // Tag names can contain `$` (valid JSX identifier characters), so
                    // escape the closing text to avoid being interpreted as a snippet
                    // placeholder/variable.
                    new_text: format!("$0{}", escape_snippet_text(&closing_text)),
                },
            }),
        })
    }
}

fn is_unclosed_tag(store: &ast::AstStore, node: &ast::Node) -> bool {
    let opening_element = store.opening_element(*node).unwrap();
    let closing_element = store.closing_element(*node).unwrap();
    let opening_tag_name = store.tag_name(opening_element).unwrap();
    let closing_tag_name = store.tag_name(closing_element).unwrap();
    if !ast::tag_names_are_equivalent(store, opening_tag_name, closing_tag_name) {
        return true;
    }

    let parent = store.parent(*node);
    if let Some(parent) = parent {
        if !ast::is_jsx_element(store, parent) {
            return false;
        }
        let parent_opening = store.opening_element(parent).unwrap();
        let parent_tag_name = store.tag_name(parent_opening).unwrap();
        return ast::tag_names_are_equivalent(store, opening_tag_name, parent_tag_name)
            && is_unclosed_tag(store, &parent);
    }

    false
}

fn is_unclosed_fragment(store: &ast::AstStore, node: &ast::Node) -> bool {
    let closing_fragment = store.closing_fragment(*node).unwrap();
    if store
        .flags(closing_fragment)
        .contains(ast::NodeFlags::THIS_NODE_HAS_ERROR)
    {
        return true;
    }

    let parent = store.parent(*node);
    if let Some(parent) = parent {
        if ast::is_jsx_fragment(store, parent) && is_unclosed_fragment(store, &parent) {
            return true;
        }
    }

    false
}

fn escape_snippet_text(text: &str) -> String {
    text.replace('$', r"\$")
}

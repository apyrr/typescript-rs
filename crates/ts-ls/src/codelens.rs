use serde_json::json;
use ts_ast as ast;
use ts_compiler as compiler;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_locale as locale;
use ts_lsproto as lsproto;
use ts_scanner as scanner;

use crate::findallreferences::SymbolEntryTransformOptions;
use crate::lsutil;
use crate::utilities::source_node_symbol_from_program;
use crate::{CrossProjectOrchestrator, LanguageService};

impl LanguageService<'_> {
    pub fn provide_code_lenses(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
    ) -> Result<lsproto::CodeLensResponse, core::Error> {
        let (program, file) = self.get_program_and_file(document_uri.clone());

        let user_prefs = self.user_preferences().code_lens;
        if !user_prefs.references_code_lens_enabled.is_true()
            && !user_prefs.implementations_code_lens_enabled.is_true()
        {
            return Ok(lsproto::CodeLensResponse::default());
        }

        // Keeps track of the last symbol to avoid duplicating code lenses across overloads.
        let mut last_symbol: Option<ast::SymbolIdentity> = None;
        let mut result = Vec::new();
        fn visit(
            l: &LanguageService<'_>,
            ctx: &core::Context,
            document_uri: &lsproto::DocumentUri,
            program: &compiler::Program,
            file: &ast::SourceFile,
            user_prefs: &lsutil::CodeLensUserPreferences,
            last_symbol: &mut Option<ast::SymbolIdentity>,
            result: &mut Vec<lsproto::CodeLens>,
            node: &ast::Node,
        ) -> bool {
            if ctx.err().is_some() {
                return true;
            }

            let store = file.store();
            let current_symbol = source_node_symbol_from_program(program, file, *node);
            if *last_symbol != current_symbol {
                *last_symbol = current_symbol;

                if user_prefs.references_code_lens_enabled.is_true()
                    && is_valid_reference_lens_node(store, node, user_prefs.clone())
                {
                    result.push(l.new_code_lens_for_node(
                        document_uri.clone(),
                        file,
                        node,
                        lsproto::CodeLensKind::References,
                    ));
                }

                if user_prefs.implementations_code_lens_enabled.is_true()
                    && is_valid_implementations_code_lens_node(store, node, user_prefs.clone())
                {
                    result.push(l.new_code_lens_for_node(
                        document_uri.clone(),
                        file,
                        node,
                        lsproto::CodeLensKind::Implementations,
                    ));
                }
            }

            let saved_last_symbol = *last_symbol;
            let _ = store.for_each_present_child(*node, |child| {
                visit(
                    l,
                    ctx,
                    document_uri,
                    program,
                    file,
                    user_prefs,
                    last_symbol,
                    result,
                    &child,
                );
                std::ops::ControlFlow::Continue(())
            });
            *last_symbol = saved_last_symbol;
            false
        }

        visit(
            self,
            ctx,
            &document_uri,
            program,
            file,
            &user_prefs,
            &mut last_symbol,
            &mut result,
            &file.as_node(),
        );

        Ok(lsproto::CodeLensResponse {
            code_lenses: Some(result.into_iter().map(Some).collect()),
        })
    }

    pub fn resolve_code_lens(
        &self,
        ctx: &core::Context,
        mut code_lens: lsproto::CodeLens,
        show_locations_command_name: Option<&String>,
        orchestrator: Option<&dyn CrossProjectOrchestrator>,
    ) -> Result<lsproto::CodeLens, core::Error> {
        let uri = code_lens.data.as_ref().unwrap().uri.clone();
        let loc = locale::und();
        let mut locs = Vec::new();
        let lens_title;
        match code_lens.data.as_ref().unwrap().kind {
            lsproto::CodeLensKind::References => {
                let references_resp = self.provide_references(
                    ctx,
                    &serde_json::from_value(json!({
                        "textDocument": { "uri": uri },
                        "position": code_lens.range.start,
                        "context": {
                            // Don't include the declaration in the references count.
                            "includeDeclaration": false,
                        },
                    }))
                    .map_err(|err| core::Error::new(err.to_string()))?,
                    orchestrator,
                )?;
                if let Some(locations) = references_resp.locations {
                    locs = locations;
                }

                if locs.len() == 1 {
                    lens_title = diagnostics::X_1_REFERENCE.localize(loc.clone(), vec![]);
                } else {
                    lens_title = diagnostics::X_0_REFERENCES
                        .localize(loc.clone(), vec![Box::new(locs.len())]);
                }
            }
            lsproto::CodeLensKind::Implementations => {
                let implementations = self.provide_implementations_ex(
                    ctx,
                    &serde_json::from_value(json!({
                        "textDocument": { "uri": uri },
                        "position": code_lens.range.start,
                    }))
                    .map_err(|err| core::Error::new(err.to_string()))?,
                    // "Force" link support to be false so that we only get `Locations` back,
                    // and don't include the "current" node in the results.
                    SymbolEntryTransformOptions {
                        require_locations_result: true,
                        drop_origin_nodes: true,
                        ..Default::default()
                    },
                    orchestrator,
                )?;

                if let Some(locations) = implementations.locations {
                    locs = locations;
                }

                if locs.len() == 1 {
                    lens_title = diagnostics::X_1_IMPLEMENTATION.localize(loc.clone(), vec![]);
                } else {
                    lens_title = diagnostics::X_0_IMPLEMENTATIONS
                        .localize(loc.clone(), vec![Box::new(locs.len())]);
                }
            }
        }

        let mut cmd = lsproto::Command {
            title: lens_title,
            ..Default::default()
        };
        if !locs.is_empty() && show_locations_command_name.is_some() {
            cmd.command = show_locations_command_name.unwrap().clone();
            cmd.arguments = Some(vec![json!(uri), json!(code_lens.range.start), json!(locs)]);
        }

        code_lens.command = Some(cmd);
        Ok(code_lens)
    }

    pub fn new_code_lens_for_node(
        &self,
        file_uri: lsproto::DocumentUri,
        file: &ast::SourceFile,
        node: &ast::Node,
        kind: lsproto::CodeLensKind,
    ) -> lsproto::CodeLens {
        let node_name = file.store().name(*node);
        let node_for_range = node_name.as_ref().unwrap_or(node);
        let pos = scanner::skip_trivia(
            file.text(),
            file.store().loc(*node_for_range).pos() as usize,
        );

        lsproto::CodeLens {
            range: lsproto::Range {
                start: self
                    .converters
                    .position_to_line_and_character(file, core::TextPos(pos as i32)),
                end: self.converters.position_to_line_and_character(
                    file,
                    core::TextPos(file.store().loc(*node).end()),
                ),
            },
            data: Some(lsproto::CodeLensData {
                kind,
                uri: file_uri,
            }),
            ..Default::default()
        }
    }
}

pub fn is_valid_implementations_code_lens_node(
    store: &ast::AstStore,
    node: &ast::Node,
    user_prefs: lsutil::CodeLensUserPreferences,
) -> bool {
    match store.kind(*node) {
        // Always show on interfaces
        ast::Kind::InterfaceDeclaration => {
            // TODO: ast.KindTypeAliasDeclaration?
            true
        }

        // If configured, show on interface methods
        ast::Kind::MethodSignature => {
            user_prefs
                .implementations_code_lens_show_on_interface_methods
                .is_true()
                && store
                    .parent(*node)
                    .is_some_and(|parent| store.kind(parent) == ast::Kind::InterfaceDeclaration)
        }

        // If configured, show on all class methods - but not private ones.
        ast::Kind::MethodDeclaration => {
            if user_prefs
                .implementations_code_lens_show_on_all_class_methods
                .is_true()
                && store
                    .parent(*node)
                    .is_some_and(|parent| store.kind(parent) == ast::Kind::ClassDeclaration)
            {
                return !ast::has_modifier(store, node, ast::MODIFIER_FLAGS_PRIVATE)
                    && store
                        .name(*node)
                        .is_none_or(|name| store.kind(name) != ast::Kind::PrivateIdentifier);
            }
            ast::has_modifier(store, node, ast::MODIFIER_FLAGS_ABSTRACT)
        }

        // Always show on abstract classes/properties/methods
        ast::Kind::ClassDeclaration
        | ast::Kind::Constructor
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor
        | ast::Kind::PropertyDeclaration => {
            ast::has_modifier(store, node, ast::MODIFIER_FLAGS_ABSTRACT)
        }

        _ => false,
    }
}

pub fn is_valid_reference_lens_node(
    store: &ast::AstStore,
    node: &ast::Node,
    user_prefs: lsutil::CodeLensUserPreferences,
) -> bool {
    match store.kind(*node) {
        ast::Kind::FunctionDeclaration => {
            if user_prefs
                .references_code_lens_show_on_all_functions
                .is_true()
            {
                return true;
            }
            ast::get_combined_modifier_flags(store, *node) & ast::MODIFIER_FLAGS_EXPORT
                != ast::MODIFIER_FLAGS_NONE
        }

        ast::Kind::VariableDeclaration => {
            ast::get_combined_modifier_flags(store, *node) & ast::MODIFIER_FLAGS_EXPORT
                != ast::MODIFIER_FLAGS_NONE
        }

        ast::Kind::ClassDeclaration
        | ast::Kind::InterfaceDeclaration
        | ast::Kind::TypeAliasDeclaration
        | ast::Kind::EnumDeclaration
        | ast::Kind::EnumMember => true,

        ast::Kind::MethodDeclaration
        | ast::Kind::MethodSignature
        | ast::Kind::Constructor
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor
        | ast::Kind::PropertyDeclaration
        | ast::Kind::PropertySignature => {
            // Don't show if child and parent have same start
            // For https://github.com/microsoft/vscode/issues/90396
            // !!!

            matches!(
                store.parent(*node).map(|parent| store.kind(parent)),
                Some(ast::Kind::ClassDeclaration)
                    | Some(ast::Kind::InterfaceDeclaration)
                    | Some(ast::Kind::TypeLiteral)
            )
        }

        _ => false,
    }
}

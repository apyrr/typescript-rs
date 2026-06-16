use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_debug as debug;
use ts_lsproto as lsproto;
use ts_modulespecifiers::CheckerShape;
use ts_nodebuilder as nodebuilder;
use ts_printer as printer;
use ts_scanner as scanner;

use crate::LanguageService;
use crate::completions::is_in_string_or_regular_expression_or_template_literal as is_in_string;
use crate::format::is_in_comment;
use crate::utilities::{
    find_containing_list, get_children_with_tokens, get_possible_generic_signatures,
    get_possible_type_arguments_info,
};

#[derive(Clone, Copy)]
pub(crate) struct CallInvocation {
    pub(crate) node: ast::Node,
}

#[derive(Clone, Copy)]
pub(crate) struct TypeArgsInvocation {
    pub(crate) called: ast::Node,
}

#[derive(Clone)]
pub(crate) struct ContextualInvocation {
    pub(crate) signature: checker::SignatureHandle,
    pub(crate) node: ast::Node,
    pub(crate) symbol: ast::SymbolIdentity,
}

#[derive(Clone, Default)]
pub(crate) struct Invocation {
    pub(crate) call_invocation: Option<CallInvocation>,
    pub(crate) type_args_invocation: Option<TypeArgsInvocation>,
    pub(crate) contextual_invocation: Option<ContextualInvocation>,
}

impl Invocation {
    fn is_empty(self) -> bool {
        self.call_invocation.is_none()
            && self.type_args_invocation.is_none()
            && self.contextual_invocation.is_none()
    }
}

impl LanguageService<'_> {
    pub fn provide_signature_help(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
        position: lsproto::Position,
        context: Option<&lsproto::SignatureHelpContext>,
    ) -> Result<lsproto::SignatureHelpResponse, core::Error> {
        let (program, source_file) = self.get_program_and_file(document_uri);
        let items = self.get_signature_help_items(
            ctx,
            self.converters
                .line_and_character_to_position(source_file, position) as i32,
            program,
            source_file,
            context,
        )?;
        Ok(lsproto::SignatureHelpOrNull {
            signature_help: items,
        })
    }

    pub fn get_signature_help_items<'a>(
        &self,
        ctx: &core::Context,
        position: i32,
        program: &'a compiler::Program,
        source_file: &'a ast::SourceFile,
        context: Option<&lsproto::SignatureHelpContext>,
    ) -> Result<Option<lsproto::SignatureHelp>, core::Error> {
        program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            source_file,
            |type_checker| {
                let Some(starting_token) = astnav::find_preceding_token(source_file, position)
                else {
                    return Ok(None);
                };
                #[derive(Clone, Copy, Eq, PartialEq)]
                enum SignatureHelpTriggerReasonKind {
                    None,
                    Invoked,
                    CharacterTyped,
                    Retriggered,
                }

                let trigger_reason_kind = if let Some(context) = context {
                    match context.trigger_kind {
                        lsproto::SignatureHelpTriggerKind::TRIGGER_CHARACTER => {
                            if context.trigger_character.is_some() {
                                if context.is_retrigger {
                                    SignatureHelpTriggerReasonKind::Retriggered
                                } else {
                                    SignatureHelpTriggerReasonKind::CharacterTyped
                                }
                            } else {
                                SignatureHelpTriggerReasonKind::Invoked
                            }
                        }
                        lsproto::SignatureHelpTriggerKind::CONTENT_CHANGE => {
                            if context.is_retrigger {
                                SignatureHelpTriggerReasonKind::Retriggered
                            } else {
                                SignatureHelpTriggerReasonKind::CharacterTyped
                            }
                        }
                        lsproto::SignatureHelpTriggerKind::INVOKED => {
                            SignatureHelpTriggerReasonKind::Invoked
                        }
                        _ => SignatureHelpTriggerReasonKind::Invoked,
                    }
                } else {
                    SignatureHelpTriggerReasonKind::None
                };

                let only_use_syntactic_owners =
                    trigger_reason_kind == SignatureHelpTriggerReasonKind::CharacterTyped;
                if only_use_syntactic_owners
                    && (is_in_string(source_file.store(), &starting_token, position)
                        || is_in_comment(source_file, position, Some(&starting_token)).is_some())
                {
                    return Ok(None);
                }

                let is_manually_invoked =
                    trigger_reason_kind == SignatureHelpTriggerReasonKind::Invoked;
                let argument_info = get_containing_argument_info(
                    &starting_token,
                    source_file,
                    type_checker,
                    is_manually_invoked,
                    position,
                );
                let Some(argument_info) = argument_info else {
                    return Ok(None);
                };

                if ctx.err().is_some() {
                    return Ok(None);
                }

                let candidate_info = get_candidate_or_type_info(
                    &argument_info,
                    type_checker,
                    source_file,
                    &starting_token,
                    only_use_syntactic_owners,
                );

                if ctx.err().is_some() {
                    return Ok(None);
                }

                let result = if let Some(candidate_info) = candidate_info {
                    if let Some(candidate_info) = candidate_info.candidate_info {
                        self.create_signature_help_items(
                            ctx,
                            &candidate_info.candidates,
                            candidate_info.resolved_signature,
                            &argument_info,
                            source_file,
                            type_checker,
                            only_use_syntactic_owners,
                        )
                    } else if let Some(type_info) = candidate_info.type_info {
                        create_type_help_items(
                            ctx,
                            type_info,
                            &argument_info,
                            source_file,
                            type_checker,
                        )
                    } else {
                        None
                    }
                } else if ast::is_source_file_js(source_file) {
                    self.create_js_signature_help_items(ctx, &argument_info, program, type_checker)
                } else {
                    None
                };
                Ok(result)
            },
        )
    }

    pub(crate) fn create_js_signature_help_items<'a>(
        &self,
        ctx: &core::Context,
        argument_info: &ArgumentListInfo,
        program: &'a compiler::Program,
        checker: &mut checker::Checker<'a, '_>,
    ) -> Option<lsproto::SignatureHelp> {
        if argument_info.invocation.contextual_invocation.is_some() {
            return None;
        }
        let expression_store = checker
            .source_file_store(
                argument_info
                    .invocation
                    .call_invocation
                    .as_ref()
                    .map(|call| call.node)
                    .or_else(|| {
                        argument_info
                            .invocation
                            .type_args_invocation
                            .as_ref()
                            .map(|type_args| type_args.called)
                    })?,
            )
            .expect("signature help invocation should belong to checker source file");
        let expression = get_expression_from_invocation(&expression_store, argument_info)?;
        if !ast::is_property_access_expression(expression_store, expression) {
            return None;
        }
        let name = expression_store
            .name(expression)
            .map(|name| expression_store.text(name))
            .unwrap_or_default();
        if name.is_empty() {
            return None;
        }

        for source_file in program.get_parsed_source_files_refs() {
            let result = self.find_signature_help_from_named_declarations(
                ctx,
                source_file,
                &name,
                argument_info,
                checker,
            );
            if result.is_some() {
                return result;
            }
        }
        None
    }

    pub(crate) fn find_signature_help_from_named_declarations<'a>(
        &self,
        ctx: &core::Context,
        source_file: &'a ast::SourceFile,
        name: &str,
        argument_info: &ArgumentListInfo,
        checker: &mut checker::Checker<'a, '_>,
    ) -> Option<lsproto::SignatureHelp> {
        let mut result = None;
        fn visit<'a>(
            service: &LanguageService<'_>,
            ctx: &core::Context,
            source_file: &'a ast::SourceFile,
            name: &str,
            argument_info: &ArgumentListInfo,
            checker: &mut checker::Checker<'a, '_>,
            result: &mut Option<lsproto::SignatureHelp>,
            node: ast::Node,
        ) -> bool {
            if result.is_some() {
                return true;
            }
            let store = source_file.store();
            if ast::get_declaration_name(store, node) == name {
                if let Some(symbol) = checker.source_node_symbol_public(node) {
                    if let Some(ty) =
                        checker.get_type_of_symbol_identity_at_location_public(symbol, Some(node))
                    {
                        let call_signatures = checker.get_call_signatures(ty);
                        if !call_signatures.is_empty() {
                            *result = service.create_signature_help_items(
                                ctx,
                                &call_signatures,
                                Some(call_signatures[0]),
                                argument_info,
                                source_file,
                                checker,
                                true,
                            );
                            if result.is_some() {
                                return true;
                            }
                        }
                    }
                }
            }
            let _ = source_file
                .store()
                .for_each_present_child(node, &mut |child: ast::Node| {
                    visit(
                        service,
                        ctx,
                        source_file,
                        name,
                        argument_info,
                        checker,
                        result,
                        child,
                    );
                    if result.is_some() {
                        std::ops::ControlFlow::Break(())
                    } else {
                        std::ops::ControlFlow::Continue(())
                    }
                });
            result.is_some()
        }
        visit(
            self,
            ctx,
            source_file,
            name,
            argument_info,
            checker,
            &mut result,
            source_file.as_node(),
        );
        result
    }

    pub(crate) fn create_signature_help_items<'a>(
        &self,
        ctx: &core::Context,
        candidates: &[checker::SignatureHandle],
        resolved_signature: Option<checker::SignatureHandle>,
        argument_info: &ArgumentListInfo,
        source_file: &'a ast::SourceFile,
        checker: &mut checker::Checker<'a, '_>,
        use_full_prefix: bool,
    ) -> Option<lsproto::SignatureHelp> {
        let caps = lsproto::get_client_capabilities(ctx);
        let doc_format = lsproto::preferred_markup_kind(
            &caps
                .text_document
                .signature_help
                .signature_information
                .documentation_format,
        );

        let enclosing_declaration =
            get_enclosing_declaration_from_invocation(argument_info.invocation.clone())?;
        let call_target_symbol =
            if let Some(contextual) = argument_info.invocation.contextual_invocation.as_ref() {
                Some(contextual.symbol)
            } else {
                let expr = get_expression_from_invocation(source_file.store(), argument_info)?;
                checker.get_symbol_at_location_public(expr).or_else(|| {
                    if use_full_prefix {
                        resolved_signature
                            .and_then(|sig| checker.signature_declaration_public(sig))
                            .and_then(|decl| checker.source_node_symbol_public(decl))
                    } else {
                        None
                    }
                })
            };

        let call_target_display_parts = if let Some(symbol) = call_target_symbol {
            if use_full_prefix {
                checker
                    .symbol_identity_to_string_ex_public(
                        symbol,
                        Some(source_file.as_node()),
                        ast::SYMBOL_FLAGS_NONE,
                        checker::SYMBOL_FORMAT_FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE,
                    )
                    .unwrap_or_default()
            } else {
                checker
                    .symbol_identity_to_string_public(symbol)
                    .unwrap_or_default()
            }
        } else {
            String::new()
        };

        let mut items = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            items.push(self.get_signature_help_item(
                *candidate,
                argument_info.is_type_parameter_list,
                &call_target_display_parts,
                enclosing_declaration,
                source_file,
                checker,
                doc_format.clone(),
            ));
        }

        let mut selected_item_index = 0_usize;
        let mut item_seen = 0_usize;
        for (i, item) in items.iter().enumerate() {
            if resolved_signature == Some(candidates[i]) {
                selected_item_index = item_seen;
                if item.len() > 1 {
                    for (count, subitem) in item.iter().enumerate() {
                        if subitem.is_variadic
                            || subitem.parameters.len() >= argument_info.argument_count as usize
                        {
                            selected_item_index = item_seen + count;
                            break;
                        }
                    }
                }
            }
            item_seen += item.len();
        }

        let flattened_signatures = items.into_iter().flatten().collect::<Vec<_>>();
        if flattened_signatures.is_empty() {
            return None;
        }

        let sig_info_caps = caps.text_document.signature_help.signature_information;
        let supports_per_signature_active_param = sig_info_caps.active_parameter_support;
        let supports_null_active_param = sig_info_caps.no_active_parameter_support;

        let mut signature_information = Vec::with_capacity(flattened_signatures.len());
        for item in &flattened_signatures {
            let parameters = item
                .parameters
                .iter()
                .map(|param| param.parameter_info.clone())
                .collect::<Vec<_>>();
            let documentation =
                item.documentation
                    .as_ref()
                    .map(|doc| lsproto::StringOrMarkupContent {
                        markup_content: Some(lsproto::MarkupContent {
                            kind: doc_format.clone(),
                            value: doc.clone(),
                        }),
                        ..Default::default()
                    });
            let mut sig_info = lsproto::SignatureInformation {
                label: item.label.clone(),
                documentation,
                parameters: Some(parameters),
                ..Default::default()
            };
            if supports_per_signature_active_param {
                sig_info.active_parameter = self.compute_active_parameter(
                    item,
                    argument_info.argument_index,
                    supports_null_active_param,
                );
            }
            signature_information.push(sig_info);
        }

        let mut help = lsproto::SignatureHelp {
            signatures: signature_information,
            active_signature: Some(selected_item_index as u32),
            ..Default::default()
        };
        if !supports_per_signature_active_param {
            help.active_parameter = self.compute_active_parameter(
                &flattened_signatures[selected_item_index],
                argument_info.argument_index,
                supports_null_active_param,
            );
        }
        Some(help)
    }

    pub fn compute_active_parameter(
        &self,
        sig: &SignatureInformation,
        argument_index: i32,
        supports_null: bool,
    ) -> Option<lsproto::UintegerOrNull> {
        let param_count = sig.parameters.len();
        if param_count == 0 {
            return None;
        }

        let mut active_param = argument_index as u32;
        if sig.is_variadic {
            let first_rest = sig.parameters.iter().position(|p| p.is_rest);
            if let Some(first_rest) = first_rest {
                if first_rest < param_count - 1 {
                    if supports_null {
                        return Some(lsproto::UintegerOrNull::default());
                    }
                    return Some(lsproto::UintegerOrNull {
                        uinteger: Some(param_count as u32),
                    });
                }
            }
            if active_param > (param_count - 1) as u32 {
                active_param = (param_count - 1) as u32;
            }
        }

        Some(lsproto::UintegerOrNull {
            uinteger: Some(active_param),
        })
    }

    pub fn get_signature_help_item<'a>(
        &self,
        candidate: checker::SignatureHandle,
        is_type_parameter_list: bool,
        call_target_symbol: &str,
        enclosing_declaration: ast::Node,
        source_file: &'a ast::SourceFile,
        checker: &mut checker::Checker<'a, '_>,
        doc_format: lsproto::MarkupKind,
    ) -> Vec<SignatureInformation> {
        let infos = if is_type_parameter_list {
            self.item_info_for_type_parameters(
                candidate,
                checker,
                enclosing_declaration,
                source_file,
                doc_format.clone(),
            )
        } else {
            self.item_info_for_parameters(
                candidate,
                checker,
                enclosing_declaration,
                source_file,
                doc_format.clone(),
            )
        };

        let suffix_display_parts = return_type_to_display_parts(candidate, checker);
        let documentation = checker
            .signature_declaration_public(candidate)
            .and_then(|decl| {
                let doc = self.get_documentation_from_declaration(
                    checker,
                    None,
                    Some(decl),
                    decl,
                    doc_format.clone(),
                    true,
                );
                if doc.is_empty() { None } else { Some(doc) }
            });

        let mut result = Vec::with_capacity(infos.len());
        for info in infos {
            let label = format!(
                "{call_target_symbol}{}{}",
                info.display_parts, suffix_display_parts
            );
            result.push(SignatureInformation {
                label,
                documentation: documentation.clone(),
                parameters: info.parameters,
                is_variadic: info.is_variadic,
            });
        }
        result
    }

    pub fn item_info_for_type_parameters<'a>(
        &self,
        candidate_signature: checker::SignatureHandle,
        checker: &mut checker::Checker<'a, '_>,
        enclosing_declaration: ast::Node,
        source_file: &'a ast::SourceFile,
        doc_format: lsproto::MarkupKind,
    ) -> Vec<SignatureHelpItemInfo> {
        let mut printer = printer::Printer::new(
            printer::PrinterOptions {
                new_line: core::NewLineKind::LF,
                ..Default::default()
            },
            printer::PrintHandlers::default(),
            None,
        );

        let type_parameters = checker
            .signature_target_public(candidate_signature)
            .map(|target| checker.signature_type_parameters_public(target))
            .unwrap_or_else(|| checker.signature_type_parameters_public(candidate_signature));
        let signature_help_type_parameters = type_parameters
            .iter()
            .map(|type_parameter| {
                create_signature_help_parameter_for_type_parameter(
                    *type_parameter,
                    source_file,
                    enclosing_declaration,
                    checker,
                    &mut printer,
                )
            })
            .collect::<Vec<_>>();

        let mut this_parameter = Vec::new();
        if let Some(this_parameter_symbol) =
            checker.signature_this_parameter_public(candidate_signature)
        {
            this_parameter.push(self.create_signature_help_parameter_for_parameter(
                this_parameter_symbol,
                enclosing_declaration,
                &mut printer,
                source_file,
                checker,
                doc_format.clone(),
            ));
        }

        let mut display_parts = scanner::token_to_string(ast::Kind::LessThanToken);
        for (i, type_parameter) in signature_help_type_parameters.iter().enumerate() {
            if i > 0 {
                display_parts.push_str(", ");
            }
            display_parts.push_str(
                type_parameter
                    .parameter_info
                    .label
                    .string
                    .as_deref()
                    .unwrap_or_default(),
            );
        }
        display_parts.push_str(&scanner::token_to_string(ast::Kind::GreaterThanToken));

        let lists = checker.get_expanded_parameters_public(candidate_signature, false);
        if !lists.is_empty() {
            display_parts.push_str(&scanner::token_to_string(ast::Kind::OpenParenToken));
        }

        let mut result = Vec::with_capacity(lists.len());
        for parameter_list in lists {
            let mut display_parameters = display_parts.clone();
            let mut parameters = this_parameter.clone();
            for (j, param) in parameter_list.iter().enumerate() {
                let parameter = self.create_signature_help_parameter_for_parameter(
                    param.clone(),
                    enclosing_declaration,
                    &mut printer,
                    source_file,
                    checker,
                    doc_format.clone(),
                );
                parameters.push(parameter.clone());
                if j > 0 {
                    display_parameters.push_str(", ");
                }
                display_parameters.push_str(
                    parameter
                        .parameter_info
                        .label
                        .string
                        .as_deref()
                        .unwrap_or_default(),
                );
            }
            display_parameters.push_str(&scanner::token_to_string(ast::Kind::CloseParenToken));
            result.push(SignatureHelpItemInfo {
                is_variadic: false,
                parameters: signature_help_type_parameters.clone(),
                display_parts: display_parameters,
            });
        }
        result
    }

    pub fn item_info_for_parameters<'a>(
        &self,
        candidate_signature: checker::SignatureHandle,
        checker: &mut checker::Checker<'a, '_>,
        enclosing_declaration: ast::Node,
        source_file: &'a ast::SourceFile,
        doc_format: lsproto::MarkupKind,
    ) -> Vec<SignatureHelpItemInfo> {
        let mut printer = printer::Printer::new(
            printer::PrinterOptions {
                new_line: core::NewLineKind::LF,
                ..Default::default()
            },
            printer::PrintHandlers::default(),
            None,
        );

        let signature_help_type_parameters = checker
            .signature_type_parameters_public(candidate_signature)
            .iter()
            .map(|type_parameter| {
                create_signature_help_parameter_for_type_parameter(
                    *type_parameter,
                    source_file,
                    enclosing_declaration,
                    checker,
                    &mut printer,
                )
            })
            .collect::<Vec<_>>();

        let mut display_parts = String::new();
        if !signature_help_type_parameters.is_empty() {
            display_parts.push_str(&scanner::token_to_string(ast::Kind::LessThanToken));
            for (i, type_parameter) in signature_help_type_parameters.iter().enumerate() {
                if i > 0 {
                    display_parts.push_str(", ");
                }
                display_parts.push_str(
                    type_parameter
                        .parameter_info
                        .label
                        .string
                        .as_deref()
                        .unwrap_or_default(),
                );
            }
            display_parts.push_str(&scanner::token_to_string(ast::Kind::GreaterThanToken));
        }

        let lists = checker.get_expanded_parameters_public(candidate_signature, false);
        if !lists.is_empty() {
            display_parts.push_str(&scanner::token_to_string(ast::Kind::OpenParenToken));
        }

        let mut result = Vec::with_capacity(lists.len());
        let list_count = lists.len();
        for parameter_list in lists {
            let is_variadic = if !checker.has_effective_rest_parameter_public(candidate_signature) {
                false
            } else if list_count == 1 {
                true
            } else {
                parameter_list.last().is_some_and(|param| {
                    checker
                        .symbol_check_flags_public(*param)
                        .is_some_and(|flags| flags & ast::CHECK_FLAGS_REST_PARAMETER != 0)
                })
            };

            let mut parameters = Vec::with_capacity(parameter_list.len());
            let mut display_parameters = display_parts.clone();
            for (j, param) in parameter_list.iter().enumerate() {
                let parameter = self.create_signature_help_parameter_for_parameter(
                    param.clone(),
                    enclosing_declaration,
                    &mut printer,
                    source_file,
                    checker,
                    doc_format.clone(),
                );
                parameters.push(parameter.clone());
                if j > 0 {
                    display_parameters.push_str(", ");
                }
                display_parameters.push_str(
                    parameter
                        .parameter_info
                        .label
                        .string
                        .as_deref()
                        .unwrap_or_default(),
                );
            }
            display_parameters.push_str(&scanner::token_to_string(ast::Kind::CloseParenToken));
            result.push(SignatureHelpItemInfo {
                is_variadic,
                parameters,
                display_parts: display_parameters,
            });
        }
        result
    }

    pub(crate) fn create_signature_help_parameter_for_parameter<'a>(
        &self,
        parameter: ast::SymbolIdentity,
        enclosing_declaration: ast::Node,
        printer: &mut printer::Printer,
        source_file: &'a ast::SourceFile,
        checker: &mut checker::Checker<'a, '_>,
        doc_format: lsproto::MarkupKind,
    ) -> SignatureHelpParameter {
        let value_declaration = checker.symbol_value_declaration_public(parameter);
        let documentation = value_declaration.as_ref().and_then(|decl| {
            let doc = self.get_documentation_from_declaration(
                checker,
                None,
                Some(*decl),
                *decl,
                doc_format.clone(),
                true,
            );
            if doc.is_empty() {
                None
            } else {
                Some(lsproto::StringOrMarkupContent {
                    markup_content: Some(lsproto::MarkupContent {
                        kind: doc_format.clone(),
                        value: doc,
                    }),
                    ..Default::default()
                })
            }
        });
        let display_node = checker
            .symbol_identity_to_parameter_declaration_public(
                parameter,
                Some(enclosing_declaration),
                SIGNATURE_HELP_NODE_BUILDER_FLAGS,
                nodebuilder::INTERNAL_FLAGS_NONE,
            )
            .expect("symbol_to_parameter_declaration returned no node");
        let display = printer.emit(&display_node, Some(source_file));
        SignatureHelpParameter {
            parameter_info: lsproto::ParameterInformation {
                label: lsproto::StringOrTuple::from_string(display),
                documentation,
                ..Default::default()
            },
            is_rest: checker
                .symbol_check_flags_public(parameter)
                .is_some_and(|flags| flags & ast::CHECK_FLAGS_REST_PARAMETER != 0),
            is_optional: checker
                .symbol_check_flags_public(parameter)
                .is_some_and(|flags| flags & ast::CHECK_FLAGS_OPTIONAL_PARAMETER != 0),
        }
    }
}

pub const SIGNATURE_HELP_NODE_BUILDER_FLAGS: nodebuilder::Flags =
    nodebuilder::FLAGS_OMIT_PARAMETER_MODIFIERS
        | nodebuilder::FLAGS_IGNORE_ERRORS
        | nodebuilder::FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE;

#[derive(Clone, Default)]
pub struct SignatureInformation {
    pub label: String,
    pub documentation: Option<String>,
    pub parameters: Vec<SignatureHelpParameter>,
    pub is_variadic: bool,
}

#[derive(Clone, Default)]
pub struct SignatureHelpItemInfo {
    pub is_variadic: bool,
    pub parameters: Vec<SignatureHelpParameter>,
    pub display_parts: String,
}

#[derive(Clone, Default)]
pub struct SignatureHelpParameter {
    pub parameter_info: lsproto::ParameterInformation,
    pub is_rest: bool,
    pub is_optional: bool,
}

#[derive(Clone)]
struct CandidateInfo {
    candidates: Vec<checker::SignatureHandle>,
    resolved_signature: Option<checker::SignatureHandle>,
}

#[derive(Clone, Default)]
struct CandidateOrTypeInfo {
    candidate_info: Option<CandidateInfo>,
    type_info: Option<ast::SymbolIdentity>,
}

pub(crate) fn create_type_help_items<'a>(
    ctx: &core::Context,
    symbol: ast::SymbolIdentity,
    argument_info: &ArgumentListInfo,
    source_file: &'a ast::SourceFile,
    checker: &mut checker::Checker<'a, '_>,
) -> Option<lsproto::SignatureHelp> {
    let (type_parameters, label) = {
        let type_parameters = checker.get_local_type_parameters_of_symbol_identity_public(symbol);
        let label = checker.symbol_identity_to_string_public(symbol)?;
        (type_parameters, label)
    };
    if type_parameters.is_empty() {
        return None;
    }
    let enclosing_declaration =
        get_enclosing_declaration_from_invocation(argument_info.invocation.clone())?;
    let item = get_type_help_item(
        label,
        &type_parameters,
        enclosing_declaration,
        source_file,
        checker,
    );

    let caps = lsproto::get_client_capabilities(ctx);
    let sig_info_caps = caps.text_document.signature_help.signature_information;
    let supports_per_signature_active_param = sig_info_caps.active_parameter_support;

    let parameters = item
        .parameters
        .iter()
        .map(|param| param.parameter_info.clone())
        .collect::<Vec<_>>();

    let mut sig_info = lsproto::SignatureInformation {
        label: item.label,
        documentation: None,
        parameters: Some(parameters),
        ..Default::default()
    };
    if supports_per_signature_active_param && !item.parameters.is_empty() {
        sig_info.active_parameter = Some(lsproto::UintegerOrNull {
            uinteger: Some(argument_info.argument_index as u32),
        });
    }

    let mut help = lsproto::SignatureHelp {
        signatures: vec![sig_info],
        active_signature: Some(0),
        ..Default::default()
    };
    if !supports_per_signature_active_param && !item.parameters.is_empty() {
        help.active_parameter = Some(lsproto::UintegerOrNull {
            uinteger: Some(argument_info.argument_index as u32),
        });
    }
    Some(help)
}

fn get_type_help_item<'a>(
    mut label: String,
    type_parameters: &[checker::TypeHandle],
    enclosing_declaration: ast::Node,
    source_file: &'a ast::SourceFile,
    checker: &mut checker::Checker<'a, '_>,
) -> SignatureInformation {
    let mut printer = printer::Printer::new(
        printer::PrinterOptions {
            new_line: core::NewLineKind::LF,
            ..Default::default()
        },
        printer::PrintHandlers::default(),
        None,
    );
    let parameters = type_parameters
        .iter()
        .map(|type_parameter| {
            create_signature_help_parameter_for_type_parameter(
                *type_parameter,
                source_file,
                enclosing_declaration,
                checker,
                &mut printer,
            )
        })
        .collect::<Vec<_>>();

    if !parameters.is_empty() {
        label.push_str(&scanner::token_to_string(ast::Kind::LessThanToken));
        for (i, parameter) in parameters.iter().enumerate() {
            if i > 0 {
                label.push_str(", ");
            }
            label.push_str(
                parameter
                    .parameter_info
                    .label
                    .string
                    .as_deref()
                    .unwrap_or_default(),
            );
        }
        label.push_str(&scanner::token_to_string(ast::Kind::GreaterThanToken));
    }

    SignatureInformation {
        label,
        documentation: None,
        parameters,
        is_variadic: false,
    }
}

pub fn create_signature_help_parameter_for_type_parameter<'a>(
    ty: checker::TypeHandle,
    source_file: &'a ast::SourceFile,
    enclosing_declaration: ast::Node,
    checker: &mut checker::Checker<'a, '_>,
    printer: &mut printer::Printer,
) -> SignatureHelpParameter {
    let display_node = checker
        .type_parameter_to_declaration_public(
            ty,
            Some(enclosing_declaration),
            SIGNATURE_HELP_NODE_BUILDER_FLAGS,
            nodebuilder::INTERNAL_FLAGS_NONE,
        )
        .expect("type_parameter_to_declaration returned no node");
    let display = printer.emit(&display_node, Some(source_file));
    SignatureHelpParameter {
        parameter_info: lsproto::ParameterInformation {
            label: lsproto::StringOrTuple::from_string(display),
            ..Default::default()
        },
        is_rest: false,
        is_optional: false,
    }
}

fn get_enclosing_declaration_from_invocation(invocation: Invocation) -> Option<ast::Node> {
    if let Some(call_invocation) = invocation.call_invocation {
        Some(call_invocation.node)
    } else if let Some(type_args_invocation) = invocation.type_args_invocation {
        Some(type_args_invocation.called)
    } else {
        invocation
            .contextual_invocation
            .map(|contextual| contextual.node)
    }
}

fn get_expression_from_invocation(
    store: &ast::AstStore,
    argument_info: &ArgumentListInfo,
) -> Option<ast::Node> {
    if let Some(call_invocation) = argument_info.invocation.call_invocation {
        ast::get_invoked_expression(store, &call_invocation.node)
    } else {
        argument_info
            .invocation
            .type_args_invocation
            .map(|invocation| invocation.called)
    }
}

fn get_candidate_or_type_info<'a>(
    info: &ArgumentListInfo,
    checker: &mut checker::Checker<'a, '_>,
    source_file: &'a ast::SourceFile,
    starting_token: &ast::Node,
    only_use_syntactic_owners: bool,
) -> Option<CandidateOrTypeInfo> {
    if let Some(call_invocation) = info.invocation.call_invocation {
        if only_use_syntactic_owners
            && !is_syntactic_owner(*starting_token, call_invocation.node, source_file)
        {
            return None;
        }
        let (resolved_signature, candidates) = checker::get_resolved_signature_for_signature_help(
            call_invocation.node,
            info.argument_count as usize,
            checker,
        );
        if candidates.is_empty() {
            return None;
        }
        return Some(CandidateOrTypeInfo {
            candidate_info: Some(CandidateInfo {
                candidates,
                resolved_signature,
            }),
            type_info: None,
        });
    }

    if let Some(type_args_invocation) = info.invocation.type_args_invocation {
        let called = type_args_invocation.called;
        let container_storage;
        let container = if ast::is_identifier(source_file.store(), called) {
            container_storage = source_file.store().parent(called);
            container_storage.unwrap_or(called)
        } else {
            called
        };
        if only_use_syntactic_owners
            && !contains_preceding_token(*starting_token, source_file, container)
        {
            return None;
        }

        let candidates = get_possible_generic_signatures(
            source_file.store(),
            &called,
            info.argument_count as usize,
            checker,
        );
        if !candidates.is_empty() {
            return Some(CandidateOrTypeInfo {
                candidate_info: Some(CandidateInfo {
                    resolved_signature: Some(candidates[0]),
                    candidates,
                }),
                type_info: None,
            });
        }

        if let Some(symbol) = checker.get_symbol_at_location_public(called) {
            return Some(CandidateOrTypeInfo {
                candidate_info: None,
                type_info: Some(symbol),
            });
        }
        return None;
    }

    if let Some(contextual_invocation) = info.invocation.contextual_invocation.as_ref() {
        return Some(CandidateOrTypeInfo {
            candidate_info: Some(CandidateInfo {
                candidates: vec![contextual_invocation.signature],
                resolved_signature: Some(contextual_invocation.signature),
            }),
            type_info: None,
        });
    }

    debug::assert(false, Some("unexpected empty invocation".to_string()));
    None
}

pub fn is_syntactic_owner(
    starting_token: ast::Node,
    node: ast::Node,
    source_file: &ast::SourceFile,
) -> bool {
    let store = source_file.store();
    if !ast::is_call_or_new_expression(store, &node) {
        return false;
    }
    let invocation_children = get_children_with_tokens(&node, source_file);
    match store.kind(starting_token) {
        ast::Kind::OpenParenToken | ast::Kind::CommaToken => invocation_children
            .iter()
            .any(|child| child.is_same_node_or_token(store, starting_token)),
        ast::Kind::LessThanToken => contains_preceding_token(
            starting_token,
            source_file,
            source_file.store().expression(node).unwrap_or(node),
        ),
        _ => false,
    }
}

pub fn contains_preceding_token(
    starting_token: ast::Node,
    source_file: &ast::SourceFile,
    container: ast::Node,
) -> bool {
    let store = source_file.store();
    let pos = store.loc(starting_token).pos();
    let mut current_parent = store.parent(starting_token);
    while let Some(parent) = current_parent {
        let preceding_token = astnav::find_preceding_token_ex(source_file, pos, Some(parent));
        if let Some(preceding_token) = preceding_token {
            let container_loc = store.loc(container);
            let preceding_loc = store.loc(preceding_token);
            return container_loc.contains(preceding_loc.pos())
                && container_loc.contains(preceding_loc.end() - 1);
        }
        current_parent = store.parent(parent);
    }
    false
}

#[derive(Clone, Default)]
pub(crate) struct ArgumentListInfo {
    pub(crate) is_type_parameter_list: bool,
    pub(crate) invocation: Invocation,
    pub(crate) arguments_span: core::TextRange,
    pub(crate) argument_index: i32,
    pub(crate) argument_count: i32,
}

fn get_containing_argument_info<'a>(
    node: &ast::Node,
    source_file: &'a ast::SourceFile,
    checker: &mut checker::Checker<'a, '_>,
    is_manually_invoked: bool,
    position: i32,
) -> Option<ArgumentListInfo> {
    let store = source_file.store();
    let mut current = Some(*node);
    while let Some(n) = current {
        if ast::is_source_file(store, n) || (!is_manually_invoked && ast::is_block(store, n)) {
            break;
        }
        if let Some(parent) = store.parent(n) {
            let parent_loc = store.loc(parent);
            let node_loc = store.loc(n);
            debug::assert(
                parent_loc.contains(node_loc.pos()) && parent_loc.contains(node_loc.end() - 1),
                Some("Not a subspan".to_string()),
            );
        }
        if let Some(argument_info) =
            get_immediately_containing_argument_or_contextual_parameter_info(
                &n,
                position,
                source_file,
                checker,
            )
        {
            return Some(argument_info);
        }
        current = store.parent(n);
    }
    None
}

fn get_immediately_containing_argument_or_contextual_parameter_info<'a>(
    node: &ast::Node,
    position: i32,
    source_file: &'a ast::SourceFile,
    checker: &mut checker::Checker<'a, '_>,
) -> Option<ArgumentListInfo> {
    try_get_parameter_info(*node, source_file, checker)
        .or_else(|| get_immediately_containing_argument_info(*node, position, source_file, checker))
}

pub(crate) fn get_immediately_containing_argument_info<'a>(
    node: ast::Node,
    position: i32,
    source_file: &'a ast::SourceFile,
    checker: &mut checker::Checker<'a, '_>,
) -> Option<ArgumentListInfo> {
    let store = source_file.store();
    let parent = store.parent(node)?;
    if ast::is_call_or_new_expression(store, &parent) {
        let info = get_argument_or_parameter_list_info(node, source_file, checker)?;
        let is_type_parameter_list = store
            .type_arguments(parent)
            .is_some_and(|list| list.pos() == info.list.pos());
        return Some(ArgumentListInfo {
            is_type_parameter_list,
            invocation: Invocation {
                call_invocation: Some(CallInvocation { node: parent }),
                ..Default::default()
            },
            arguments_span: info.arguments_span,
            argument_index: info.argument_index,
            argument_count: info.argument_count,
        });
    }

    if is_no_substitution_template_literal(store, node)
        && is_tagged_template_expression(store, parent)
        && is_inside_template_literal(node, position, source_file)
    {
        return get_argument_list_info_for_template(parent, 0, source_file);
    }

    if is_template_head(store, node)
        && store.parent(parent).is_some_and(|grandparent| {
            store.kind(grandparent) == ast::Kind::TaggedTemplateExpression
        })
    {
        let tag_expression = store.parent(parent).unwrap();
        let argument_index = if is_inside_template_literal(node, position, source_file) {
            0
        } else {
            1
        };
        return get_argument_list_info_for_template(tag_expression, argument_index, source_file);
    }

    if store
        .parent(parent)
        .and_then(|parent| store.parent(parent))
        .is_some_and(|parent| is_tagged_template_expression(store, parent))
        && ast::is_template_span(store, parent)
    {
        let tag_expression = store.parent(store.parent(parent).unwrap()).unwrap();
        if is_template_tail(store, node) && !is_inside_template_literal(node, position, source_file)
        {
            return None;
        }
        let template_expression = store.parent(parent).unwrap();
        let span_index = store
            .template_spans(template_expression)?
            .into_iter()
            .position(|span| store.loc(span) == store.loc(parent))
            .unwrap_or(0) as i32;
        let argument_index =
            get_argument_index_for_template_piece(span_index, node, position, source_file);
        return get_argument_list_info_for_template(tag_expression, argument_index, source_file);
    }

    if ast::is_jsx_opening_like_element(store, &parent) {
        let attributes = store.attributes(parent).unwrap();
        let attributes_loc = store.loc(attributes);
        let attribute_span_start = attributes_loc.pos();
        let attribute_span_end =
            scanner::skip_trivia(source_file.text(), attributes_loc.end() as usize) as i32;
        return Some(ArgumentListInfo {
            is_type_parameter_list: false,
            invocation: Invocation {
                call_invocation: Some(CallInvocation { node: parent }),
                ..Default::default()
            },
            arguments_span: core::new_text_range(attribute_span_start, attribute_span_end),
            argument_index: 0,
            argument_count: 1,
        });
    }

    if let Some(type_arg_info) = get_possible_type_arguments_info(&node, source_file) {
        let called = type_arg_info.called;
        let n_type_arguments = type_arg_info.n_type_arguments;
        return Some(ArgumentListInfo {
            is_type_parameter_list: true,
            invocation: Invocation {
                type_args_invocation: Some(TypeArgsInvocation { called }),
                ..Default::default()
            },
            arguments_span: core::new_text_range(store.loc(called).pos(), store.loc(node).end()),
            argument_index: n_type_arguments as i32,
            argument_count: n_type_arguments as i32 + 1,
        });
    }

    None
}

pub fn get_argument_index_for_template_piece(
    span_index: i32,
    node: ast::Node,
    position: i32,
    source_file: &ast::SourceFile,
) -> i32 {
    debug::assert(
        position >= source_file.store().loc(node).pos(),
        Some("Assumed 'position' could not occur before node.".to_string()),
    );
    if ast::is_template_literal_token(source_file.store(), &node) {
        if is_inside_template_literal(node, position, source_file) {
            return 0;
        }
        return span_index + 2;
    }
    span_index + 1
}

pub fn get_adjusted_node(node: ast::Node, source_file: &ast::SourceFile) -> Option<ast::Node> {
    let store = source_file.store();
    match store.kind(node) {
        ast::Kind::OpenParenToken | ast::Kind::CommaToken => Some(node),
        _ => {
            let parent = store.parent(node);
            ast::find_ancestor(store, parent, |store, n| {
                if ast::is_parameter_declaration(store, n) {
                    true
                } else if ast::is_binding_element(store, n)
                    || ast::is_object_binding_pattern(store, n)
                    || ast::is_array_binding_pattern(store, n)
                {
                    false
                } else {
                    false
                }
            })
        }
    }
}

#[derive(Clone, Copy)]
struct ContextualSignatureLocationInfo {
    contextual_type: checker::TypeHandle,
    argument_index: i32,
    argument_count: i32,
    arguments_span: core::TextRange,
}

pub fn get_spread_element_count<'a>(
    store: &ast::AstStore,
    node: ast::Node,
    checker: &mut checker::Checker<'a, '_>,
) -> i32 {
    let Some(expression) = store.expression(node) else {
        return 0;
    };
    let spread_type = checker.get_type_at_location(expression);
    if checker.is_tuple_type_public(spread_type) {
        let element_flags = checker.tuple_element_flags_public(spread_type);
        let fixed_length = checker.tuple_fixed_length_public(spread_type);
        if fixed_length == 0 {
            return 0;
        }
        let first_optional_index = element_flags
            .iter()
            .position(|f| *f & checker::ELEMENT_FLAGS_REQUIRED == 0);
        return first_optional_index
            .map(|i| i as i32)
            .unwrap_or(fixed_length as i32);
    }
    0
}

pub fn get_argument_index<'a>(
    node: ast::Node,
    arguments: ast::SourceNodeList<'a>,
    source_file: &'a ast::SourceFile,
    checker: &mut checker::Checker<'a, '_>,
) -> i32 {
    let parent = source_file.store().parent(node).unwrap();
    get_argument_index_or_count(
        source_file.store(),
        get_token_from_node_list(arguments, parent, source_file),
        Some(node),
        checker,
    )
}

pub fn get_argument_count<'a>(
    node: ast::Node,
    arguments: ast::SourceNodeList<'a>,
    source_file: &'a ast::SourceFile,
    checker: &mut checker::Checker<'a, '_>,
) -> i32 {
    let parent = source_file.store().parent(node).unwrap();
    get_argument_index_or_count(
        source_file.store(),
        get_token_from_node_list(arguments, parent, source_file),
        None,
        checker,
    )
}

#[derive(Clone, Copy)]
pub struct ArgumentListToken {
    pub node: Option<ast::Node>,
    pub kind: ast::Kind,
}

pub fn get_argument_index_or_count<'a>(
    store: &ast::AstStore,
    arguments: Vec<ArgumentListToken>,
    node: Option<ast::Node>,
    checker: &mut checker::Checker<'a, '_>,
) -> i32 {
    let mut argument_index = 0;
    let mut skip_comma = false;
    for arg in &arguments {
        if node.is_some_and(|node| arg.node.is_some_and(|arg_node| arg_node == node)) {
            if !skip_comma && arg.kind == ast::Kind::CommaToken {
                argument_index += 1;
            }
            return argument_index;
        }
        if arg
            .node
            .is_some_and(|arg_node| ast::is_spread_element(store, arg_node))
        {
            let arg_node = arg.node.unwrap();
            argument_index += get_spread_element_count(store, arg_node, checker);
            skip_comma = true;
            continue;
        }
        if arg.kind != ast::Kind::CommaToken {
            argument_index += 1;
            skip_comma = true;
            continue;
        }
        if skip_comma {
            skip_comma = false;
            continue;
        }
        argument_index += 1;
    }
    if node.is_some() {
        return argument_index;
    }
    if arguments
        .last()
        .is_some_and(|arg| arg.kind == ast::Kind::CommaToken)
    {
        argument_index + 1
    } else {
        argument_index
    }
}

#[derive(Clone, Copy)]
pub struct ArgumentOrParameterListInfo<'a> {
    pub list: ast::SourceNodeList<'a>,
    pub argument_index: i32,
    pub argument_count: i32,
    pub arguments_span: core::TextRange,
}

pub fn get_argument_or_parameter_list_info<'a>(
    node: ast::Node,
    source_file: &'a ast::SourceFile,
    checker: &mut checker::Checker<'a, '_>,
) -> Option<ArgumentOrParameterListInfo<'a>> {
    let info = get_argument_or_parameter_list_and_index(node, source_file, checker)?;
    let argument_count = get_argument_count(node, info.list, source_file, checker);
    let arguments_span = get_applicable_span_for_arguments(info.list, node, source_file);
    Some(ArgumentOrParameterListInfo {
        list: info.list,
        argument_index: info.argument_index,
        argument_count,
        arguments_span,
    })
}

pub fn get_applicable_span_for_arguments(
    argument_list: ast::SourceNodeList<'_>,
    node: ast::Node,
    source_file: &ast::SourceFile,
) -> core::TextRange {
    let applicable_span_start = argument_list.pos();
    let applicable_span_end =
        scanner::skip_trivia(source_file.text(), argument_list.end() as usize) as i32;
    if argument_list.is_empty() {
        let node_end = source_file.store().loc(node).end();
        return core::new_text_range(
            node_end,
            scanner::skip_trivia(source_file.text(), node_end as usize) as i32,
        );
    }
    core::new_text_range(applicable_span_start, applicable_span_end)
}

#[derive(Clone, Copy)]
pub struct ArgumentOrParameterListAndIndex<'a> {
    pub list: ast::SourceNodeList<'a>,
    pub argument_index: i32,
}

pub fn get_argument_or_parameter_list_and_index<'a>(
    node: ast::Node,
    source_file: &'a ast::SourceFile,
    checker: &mut checker::Checker<'a, '_>,
) -> Option<ArgumentOrParameterListAndIndex<'a>> {
    if matches!(
        source_file.store().kind(node),
        ast::Kind::LessThanToken | ast::Kind::OpenParenToken
    ) {
        let parent = source_file.store().parent(node)?;
        let list = get_child_list_that_starts_with_opener_token(source_file.store(), parent, node)?;
        return Some(ArgumentOrParameterListAndIndex {
            list,
            argument_index: 0,
        });
    }

    let list = find_containing_list(&node, source_file)?;
    Some(ArgumentOrParameterListAndIndex {
        list,
        argument_index: get_argument_index(node, list, source_file, checker),
    })
}

pub fn get_child_list_that_starts_with_opener_token<'a>(
    store: &'a ast::AstStore,
    parent: ast::Node,
    opener_token: ast::Node,
) -> Option<ast::SourceNodeList<'a>> {
    if ast::is_call_expression(store, parent) {
        if store.kind(opener_token) == ast::Kind::LessThanToken {
            return store.type_arguments(parent);
        }
        return store.arguments(parent);
    }
    if ast::is_new_expression(store, parent) {
        if store.kind(opener_token) == ast::Kind::LessThanToken {
            return store.type_arguments(parent);
        }
        return store.arguments(parent);
    }
    None
}

pub fn try_get_parameter_info<'a>(
    starting_token: ast::Node,
    source_file: &'a ast::SourceFile,
    checker: &mut checker::Checker<'a, '_>,
) -> Option<ArgumentListInfo> {
    let node = get_adjusted_node(starting_token, source_file)?;
    let info = get_contextual_signature_location_info(&node, source_file, checker)?;
    let non_nullable_contextual_type = checker.get_non_nullable_type_public(info.contextual_type);
    let symbol = checker.type_symbol_public(non_nullable_contextual_type)?;
    let signatures = checker
        .get_signatures_of_type_public(non_nullable_contextual_type, checker::SIGNATURE_KIND_CALL);
    if signatures.is_empty() {
        return None;
    }
    let signature = *signatures.last().unwrap();
    Some(ArgumentListInfo {
        is_type_parameter_list: false,
        invocation: Invocation {
            contextual_invocation: Some(ContextualInvocation {
                signature,
                node: starting_token,
                symbol: choose_better_symbol(source_file.store(), checker, symbol),
            }),
            ..Default::default()
        },
        arguments_span: info.arguments_span,
        argument_index: info.argument_index,
        argument_count: info.argument_count,
    })
}

pub(crate) fn choose_better_symbol(
    store: &ast::AstStore,
    checker: &mut checker::Checker<'_, '_>,
    symbol: ast::SymbolIdentity,
) -> ast::SymbolIdentity {
    {
        let Some(name) = checker.symbol_name_public(symbol) else {
            return symbol;
        };
        if name != ast::INTERNAL_SYMBOL_NAME_TYPE {
            return symbol;
        }
        let declarations = checker.collect_symbol_declarations_public(symbol);
        if !declarations.is_empty() {
            for declaration in &declarations {
                if ast::is_function_type_node(store, *declaration)
                    && store
                        .parent(*declaration)
                        .is_some_and(|parent| ast::can_have_symbol(store, parent))
                {
                    if let Some(parent_symbol) = store
                        .parent(*declaration)
                        .and_then(|parent| checker.source_node_symbol_public(parent))
                    {
                        return parent_symbol;
                    }
                }
            }
        }
    }
    symbol
}

fn get_contextual_signature_location_info<'a>(
    node: &ast::Node,
    source_file: &'a ast::SourceFile,
    checker: &mut checker::Checker<'a, '_>,
) -> Option<ContextualSignatureLocationInfo> {
    let store = source_file.store();
    let parent = store.parent(*node)?;
    match store.kind(parent) {
        ast::Kind::ParenthesizedExpression
        | ast::Kind::MethodDeclaration
        | ast::Kind::FunctionExpression
        | ast::Kind::ArrowFunction => {
            let info = get_argument_or_parameter_list_info(*node, source_file, checker)?;
            let contextual_type = if ast::is_method_declaration(store, parent) {
                checker.get_contextual_type_for_object_literal_element_public(
                    parent,
                    checker::CONTEXT_FLAGS_NONE,
                )
            } else {
                checker.get_contextual_type_public(parent, checker::CONTEXT_FLAGS_NONE)
            }?;
            Some(ContextualSignatureLocationInfo {
                contextual_type,
                argument_index: info.argument_index,
                argument_count: info.argument_count,
                arguments_span: info.arguments_span,
            })
        }
        ast::Kind::BinaryExpression => {
            let highest_binary = get_highest_binary(store, parent);
            let contextual_type =
                checker.get_contextual_type_public(highest_binary, checker::CONTEXT_FLAGS_NONE)?;
            if store.kind(*node) != ast::Kind::OpenParenToken {
                let argument_index = count_binary_expression_parameters(store, parent) - 1;
                let argument_count = count_binary_expression_parameters(store, highest_binary);
                let parent_loc = store.loc(parent);
                return Some(ContextualSignatureLocationInfo {
                    contextual_type,
                    argument_index,
                    argument_count,
                    arguments_span: core::new_text_range(parent_loc.pos(), parent_loc.end()),
                });
            }
            None
        }
        _ => None,
    }
}

pub fn get_highest_binary(store: &ast::AstStore, mut binary: ast::Node) -> ast::Node {
    while store
        .parent(binary)
        .is_some_and(|parent| ast::is_binary_expression(store, parent))
    {
        binary = store.parent(binary).unwrap();
    }
    binary
}

pub fn count_binary_expression_parameters(store: &ast::AstStore, binary: ast::Node) -> i32 {
    if store
        .left(binary)
        .is_some_and(|left| ast::is_binary_expression(store, left))
    {
        count_binary_expression_parameters(store, store.left(binary).unwrap()) + 1
    } else {
        2
    }
}

pub fn get_token_from_node_list<'a>(
    node_list: ast::SourceNodeList<'a>,
    node_list_parent: ast::Node,
    source_file: &'a ast::SourceFile,
) -> Vec<ArgumentListToken> {
    let mut left = node_list.pos();
    let mut node_list_index = 0;
    let node_list_nodes = node_list.into_iter().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    while left < node_list.end() {
        if node_list_nodes.len() > node_list_index
            && left
                == source_file
                    .store()
                    .loc(node_list_nodes[node_list_index])
                    .pos()
        {
            let node = node_list_nodes[node_list_index];
            tokens.push(ArgumentListToken {
                node: Some(node),
                kind: source_file.store().kind(node),
            });
            left = source_file
                .store()
                .loc(node_list_nodes[node_list_index])
                .end();
            node_list_index += 1;
        } else {
            let scanner = scanner::get_scanner_for_source_file(source_file, left as usize);
            let token = scanner.token();
            let token_end = scanner.token_end();
            let _ = node_list_parent;
            tokens.push(ArgumentListToken {
                node: None,
                kind: token,
            });
            left = token_end as i32;
        }
    }
    tokens
}

pub fn get_argument_list_info_for_template<'a>(
    tag_expression: ast::Node,
    argument_index: i32,
    source_file: &'a ast::SourceFile,
) -> Option<ArgumentListInfo> {
    let store = source_file.store();
    let template = store.template(tag_expression)?;
    let argument_count = if is_no_substitution_template_literal(store, template) {
        1
    } else {
        store.template_spans(template).unwrap().len() as i32 + 1
    };
    if argument_index != 0 {
        debug::assert(argument_index < argument_count, None);
    }
    Some(ArgumentListInfo {
        is_type_parameter_list: false,
        invocation: Invocation {
            call_invocation: Some(CallInvocation {
                node: tag_expression,
            }),
            ..Default::default()
        },
        argument_index,
        argument_count,
        arguments_span: get_applicable_range_for_tagged_template(tag_expression, source_file),
    })
}

pub fn get_applicable_range_for_tagged_template(
    tagged_template: ast::Node,
    source_file: &ast::SourceFile,
) -> core::TextRange {
    let store = source_file.store();
    let template = store.template(tagged_template).unwrap();
    let applicable_span_start = scanner::get_token_pos_of_node(&template, source_file, false);
    let mut applicable_span_end = store.loc(template).end();
    if store.kind(template) == ast::Kind::TemplateExpression {
        let template_spans = store.template_spans(template).unwrap();
        let last_span = template_spans.last().unwrap();
        let literal = store.literal(last_span).unwrap();
        let literal_loc = store.loc(literal);
        if literal_loc.end() - literal_loc.pos() == 0 {
            applicable_span_end =
                scanner::skip_trivia(source_file.text(), applicable_span_end as usize) as i32;
        }
    }
    core::new_text_range(applicable_span_start as i32, applicable_span_end)
}

pub fn return_type_to_display_parts<'a>(
    candidate_signature: checker::SignatureHandle,
    checker: &mut checker::Checker<'a, '_>,
) -> String {
    let mut return_type = String::from(": ");
    if let Some(predicate) = checker.get_type_predicate_of_signature_public(candidate_signature) {
        return_type.push_str(&checker.type_predicate_to_string_public(predicate));
    } else {
        // PORT NOTE: reshaped for borrowck; compute the return type before
        // formatting it so the mutable checker borrows do not overlap.
        let signature_return_type =
            checker.get_return_type_of_signature_public(candidate_signature);
        return_type.push_str(&checker.type_to_string_public(signature_return_type));
    }
    return_type
}

pub(crate) fn is_no_substitution_template_literal(store: &ast::AstStore, node: ast::Node) -> bool {
    store.kind(node) == ast::Kind::NoSubstitutionTemplateLiteral
}

pub(crate) fn is_tagged_template_expression(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_tagged_template_expression(store, node)
}

pub(crate) fn is_template_head(store: &ast::AstStore, node: ast::Node) -> bool {
    store.kind(node) == ast::Kind::TemplateHead
}

pub(crate) fn is_template_tail(store: &ast::AstStore, node: ast::Node) -> bool {
    store.kind(node) == ast::Kind::TemplateTail
}

pub fn is_inside_template_literal(
    node: ast::Node,
    position: i32,
    source_file: &ast::SourceFile,
) -> bool {
    source_file.store().loc(node).contains_exclusive(position)
}

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use ts_ast as ast;
use ts_core as core;

use crate::{
    AutoGenerateInfo, AutoGenerateOptions, EF_ASYNC_FUNCTION_BODY, EF_EXPORT_NAME, EF_HELPER_NAME,
    EF_LOCAL_NAME, EF_NO_COMMENTS, EF_NO_SOURCE_MAP, EF_NONE, EF_REUSE_TEMP_VARIABLE_SCOPE,
    EmitContext, GeneratedIdentifierFlags,
    emitcontext::{
        EmitContextStateRef, set_original_in_state, try_emit_node_in_state,
        with_emit_node_in_state_mut,
    },
    format_generated_name,
    helpers::{
        ADD_DISPOSABLE_RESOURCE_HELPER, ASYNC_DELEGATOR_HELPER, ASYNC_GENERATOR_HELPER,
        ASYNC_VALUES_HELPER, AWAIT_HELPER, AWAITER_HELPER, CLASS_PRIVATE_FIELD_GET_HELPER,
        CLASS_PRIVATE_FIELD_IN_HELPER, CLASS_PRIVATE_FIELD_SET_HELPER, DECORATE_HELPER,
        DISPOSE_RESOURCES_HELPER, ES_DECORATE_HELPER, EXPORT_STAR_HELPER, IMPORT_DEFAULT_HELPER,
        IMPORT_STAR_HELPER, MAKE_TEMPLATE_OBJECT_HELPER, METADATA_HELPER, PARAM_HELPER,
        PROP_KEY_HELPER, REST_HELPER, REWRITE_RELATIVE_IMPORT_EXTENSIONS_HELPER,
        RUN_INITIALIZERS_HELPER, SET_FUNCTION_NAME_HELPER,
    },
};

enum RestHelperPropertyName {
    Computed(ast::Node),
    Literal {
        text: String,
        text_source_node: ast::Node,
    },
}

pub struct NodeFactory {
    pub node_factory: ast::NodeFactory,
    state: Option<EmitContextStateRef>,
}

impl Default for NodeFactory {
    fn default() -> Self {
        Self {
            node_factory: ast::new_node_factory(ast::NodeFactoryHooks::default()),
            state: None,
        }
    }
}

impl Deref for NodeFactory {
    type Target = ast::NodeFactory;

    fn deref(&self) -> &Self::Target {
        &self.node_factory
    }
}

impl DerefMut for NodeFactory {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.node_factory
    }
}

impl NodeFactory {
    pub fn new_node_list(&mut self, nodes: impl IntoIterator<Item = ast::Node>) -> ast::NodeList {
        self.node_factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            nodes,
        )
    }

    pub fn new_modifier_list(
        &mut self,
        modifiers: impl IntoIterator<Item = ast::Node>,
    ) -> ast::ModifierList {
        self.node_factory.new_modifier_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            modifiers,
            ast::ModifierFlags::NONE,
        )
    }
}

impl ast::NodeFactoryCoercible for NodeFactory {
    fn as_node_factory(&mut self) -> &mut ast::NodeFactory {
        &mut self.node_factory
    }
}

pub fn new_node_factory(context: &mut EmitContext) -> NodeFactory {
    new_node_factory_with_state(context.state_ref())
}

pub(crate) fn new_node_factory_with_state(state: EmitContextStateRef) -> NodeFactory {
    let on_update_state = state.clone();
    let on_clone_state = state.clone();
    NodeFactory {
        node_factory: ast::new_node_factory(ast::NodeFactoryHooks {
            on_create: Some(Arc::new(super::emitcontext::on_create)),
            on_update: Some(Arc::new(move |_store, updated, original| {
                set_original_in_state(&on_update_state, &updated, &original);
            })),
            on_clone: Some(Arc::new(move |store, updated, original| {
                set_original_in_state(&on_clone_state, &updated, &original);
                if ast::is_identifier(store, updated) || ast::is_private_identifier(store, updated)
                {
                    let auto_generate = on_clone_state
                        .borrow()
                        .auto_generate
                        .get_cloned(node_key(&original));
                    if let Some(auto_generate) = auto_generate {
                        on_clone_state
                            .borrow_mut()
                            .auto_generate
                            .insert(node_key(&updated), auto_generate);
                    }
                }
            })),
        }),
        state: Some(state),
    }
}

impl NodeFactory {
    fn state(&self) -> EmitContextStateRef {
        self.state.as_ref().expect("emit context").clone()
    }

    fn request_emit_helper(&self, helper: &crate::EmitHelper) {
        if helper.scoped {
            panic!("Cannot request a scoped emit helper")
        }
        for dependency in helper.dependencies.iter().copied() {
            self.request_emit_helper(dependency);
        }
        self.state()
            .borrow_mut()
            .emit_helpers
            .add(crate::helpers::helper_key(helper));
    }

    pub fn request_run_initializers_helper(&self) {
        self.request_emit_helper(&RUN_INITIALIZERS_HELPER);
    }

    fn set_emit_flags(&self, node: &ast::Node, flags: crate::EmitFlags) {
        let state = self.state();
        let state = state.borrow();
        with_emit_node_in_state_mut(&state, node, |emit_node| emit_node.emit_flags = flags);
    }

    fn mark_emit_node(&self, node: &ast::Node, flags: crate::EmitFlags) {
        let state = self.state();
        let state = state.borrow();
        with_emit_node_in_state_mut(&state, node, |emit_node| emit_node.emit_flags |= flags);
    }

    fn emit_flags(&self, node: &ast::Node) -> crate::EmitFlags {
        let state = self.state();
        let state = state.borrow();
        try_emit_node_in_state(&state, node, |emit_node| emit_node.emit_flags).unwrap_or(EF_NONE)
    }

    fn set_node_loc(&mut self, node: ast::Node, loc: core::TextRange) -> ast::Node {
        self.node_factory.place_emit_synthetic_node(node, loc);
        node
    }

    fn copy_source_metadata(
        &mut self,
        node: ast::Node,
        source: &ast::AstStore,
        source_node: ast::Node,
    ) -> ast::Node {
        let loc = source.loc(source_node);
        let parent = source.parent(source_node);
        self.node_factory.place_emit_synthetic_node(node, loc);
        if parent.is_some_and(|parent| parent.store_id() == node.store_id()) {
            self.node_factory.link_emit_synthetic_parent(node, parent);
        }
        node
    }

    fn comment_range(&self, node: &ast::Node) -> core::TextRange {
        let state = self.state();
        let state = state.borrow();
        if let Some(comment_range) = try_emit_node_in_state(&state, node, |emit_node| {
            if emit_node.flags & super::emitcontext::EMIT_NODE_FLAGS_HAS_COMMENT_RANGE != 0 {
                Some(emit_node.comment_range)
            } else {
                None
            }
        })
        .flatten()
        {
            return comment_range;
        }
        self.node_factory.store().loc(*node)
    }

    fn source_map_range(&self, node: &ast::Node) -> core::TextRange {
        let state = self.state();
        let state = state.borrow();
        if let Some(source_map_range) = try_emit_node_in_state(&state, node, |emit_node| {
            if emit_node.flags & super::emitcontext::EMIT_NODE_FLAGS_HAS_SOURCE_MAP_RANGE != 0 {
                Some(emit_node.source_map_range)
            } else {
                None
            }
        })
        .flatten()
        {
            return source_map_range;
        }
        self.node_factory.store().loc(*node)
    }

    fn assign_comment_and_source_map_ranges(&self, to: &ast::Node, from: &ast::Node) {
        let comment_range = self.comment_range(from);
        let source_map_range = self.source_map_range(from);
        let state = self.state();
        let state = state.borrow();
        with_emit_node_in_state_mut(&state, to, |emit_node| {
            emit_node.comment_range = comment_range;
            emit_node.source_map_range = source_map_range;
            emit_node.flags |= super::emitcontext::EMIT_NODE_FLAGS_HAS_COMMENT_RANGE
                | super::emitcontext::EMIT_NODE_FLAGS_HAS_SOURCE_MAP_RANGE;
        });
    }

    fn has_auto_generate_info(&self, node: Option<&ast::Node>) -> bool {
        node.is_some_and(|node| {
            self.state()
                .borrow()
                .auto_generate
                .contains_key(node_key(node))
        })
    }

    fn get_node_for_generated_name_worker(
        &mut self,
        node: &ast::Node,
        auto_generate_id: super::AutoGenerateId,
    ) -> ast::Node {
        super::emitcontext::get_node_for_generated_name_worker_in_state(
            &self.state(),
            node,
            auto_generate_id,
        )
    }

    fn get_source_node_for_generated_name_worker(
        &mut self,
        source: &ast::AstStore,
        node: ast::Node,
        auto_generate_id: super::AutoGenerateId,
    ) -> ast::Node {
        let mut node = node;
        let mut original = super::emitcontext::original_in_state(&self.state(), &node);
        while let Some(next_original) = original {
            if next_original.store_id() != source.store_id()
                && next_original.store_id() != self.node_factory.store().store_id()
            {
                break;
            }
            node = next_original;
            let store = if node.store_id() == self.node_factory.store().store_id() {
                self.node_factory.store()
            } else {
                source
            };
            if ast::is_member_name(store, node) {
                let auto_generate = self
                    .state()
                    .borrow()
                    .auto_generate
                    .get_cloned(node_key(&node));
                if auto_generate.is_none()
                    || (auto_generate.as_ref().unwrap().flags.is_node()
                        && auto_generate.as_ref().unwrap().id != auto_generate_id)
                {
                    break;
                }
            }
            original = super::emitcontext::original_in_state(&self.state(), &node);
        }
        node
    }

    fn store_for_generated_name_node<'a>(
        &'a self,
        source: &'a ast::AstStore,
        node: ast::Node,
    ) -> &'a ast::AstStore {
        if node.store_id() == self.node_factory.store().store_id() {
            self.node_factory.store()
        } else {
            source
        }
    }

    fn generated_name_text_for_node(
        &mut self,
        source: &ast::AstStore,
        node: ast::Node,
        auto_generate_id: super::AutoGenerateId,
    ) -> String {
        {
            let store = self.store_for_generated_name_node(source, node);
            if ast::is_member_name(store, node) {
                return store.text(node);
            }
        }

        let generated_node =
            self.get_source_node_for_generated_name_worker(source, node, auto_generate_id);
        let generated_store = self.store_for_generated_name_node(source, generated_node);
        format!(
            "(generated@{})",
            ast::get_node_id(generated_store, generated_node)
        )
    }

    pub fn clone_node_with_hooks(&mut self, source: &ast::AstStore, node: ast::Node) -> ast::Node {
        if node.store_id() == self.node_factory.store().store_id() {
            return self
                .node_factory
                .deep_clone_node_in_current_store_preserve_location(node);
        }
        self.node_factory
            .deep_clone_node_from_store_preserve_location(source, node)
    }

    fn new_generated_identifier(
        &mut self,
        kind: GeneratedIdentifierFlags,
        text: &str,
        node: Option<(&ast::AstStore, ast::Node)>,
        options: AutoGenerateOptions,
    ) -> ast::Node {
        let id = super::emitcontext::NEXT_AUTO_GENERATE_ID
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        let mut text = text.to_string();

        if text.is_empty() {
            text = match node {
                None => format!("(auto@{id})"),
                Some((source, node)) => self.generated_name_text_for_node(source, node, id),
            };
            text = format_generated_name(false, options.prefix, &text, options.suffix);
        }

        let name = self.node_factory.new_identifier(&text);
        let auto_generate = AutoGenerateInfo {
            id,
            flags: kind | (options.flags & !GeneratedIdentifierFlags::KIND_MASK),
            prefix: options.prefix.to_string(),
            suffix: options.suffix.to_string(),
            node: node.map(|(_, node)| node).unwrap_or(name),
        };
        self.state()
            .borrow_mut()
            .auto_generate
            .insert(node_key(&name), auto_generate);
        name
    }

    fn new_generated_identifier_with_prefix_and_suffix(
        &mut self,
        kind: GeneratedIdentifierFlags,
        text: &str,
        node: Option<(&ast::AstStore, ast::Node)>,
        mut flags: GeneratedIdentifierFlags,
        prefix: &str,
        suffix: &str,
    ) -> ast::Node {
        if !prefix.is_empty() || !suffix.is_empty() {
            flags |= GeneratedIdentifierFlags::OPTIMISTIC;
        }

        let id = super::emitcontext::NEXT_AUTO_GENERATE_ID
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        let mut text = text.to_string();

        if text.is_empty() {
            text = match node {
                None => format!("(auto@{id})"),
                Some((source, node)) => self.generated_name_text_for_node(source, node, id),
            };
            text = format_generated_name(false, prefix, &text, suffix);
        }

        let name = self.node_factory.new_identifier(&text);
        let auto_generate = AutoGenerateInfo {
            id,
            flags: kind | (flags & !GeneratedIdentifierFlags::KIND_MASK),
            prefix: prefix.to_string(),
            suffix: suffix.to_string(),
            node: node.map(|(_, node)| node).unwrap_or(name),
        };
        self.state()
            .borrow_mut()
            .auto_generate
            .insert(node_key(&name), auto_generate);
        name
    }

    // Allocates a new temp variable name, but does not record it in the environment. It is recommended to pass this to either
    // `AddVariableDeclaration` or `AddLexicalDeclaration` to ensure it is properly tracked, if you are not otherwise handling
    // it yourself.
    pub fn new_temp_variable(&mut self) -> ast::Node {
        self.new_temp_variable_ex(AutoGenerateOptions::default())
    }

    // Allocates a new temp variable name, but does not record it in the environment. It is recommended to pass this to either
    // `AddVariableDeclaration` or `AddLexicalDeclaration` to ensure it is properly tracked, if you are not otherwise handling
    // it yourself.
    pub fn new_temp_variable_ex(&mut self, options: AutoGenerateOptions) -> ast::Node {
        self.new_generated_identifier(GeneratedIdentifierFlags::AUTO, "", None, options)
    }

    // Allocates a new loop variable name.
    pub fn new_loop_variable(&mut self) -> ast::Node {
        self.new_loop_variable_ex(AutoGenerateOptions::default())
    }

    // Allocates a new loop variable name.
    pub fn new_loop_variable_ex(&mut self, options: AutoGenerateOptions) -> ast::Node {
        self.new_generated_identifier(GeneratedIdentifierFlags::LOOP, "", None, options)
    }

    // Allocates a new unique name based on the provided text.
    pub fn new_unique_name(&mut self, text: &str) -> ast::Node {
        self.new_unique_name_ex(text, AutoGenerateOptions::default())
    }

    // Allocates a new unique name based on the provided text.
    pub fn new_unique_name_ex(&mut self, text: &str, options: AutoGenerateOptions) -> ast::Node {
        self.new_generated_identifier(GeneratedIdentifierFlags::UNIQUE, text, None, options)
    }

    // Allocates a new unique name based on the provided node.
    pub fn new_generated_name_for_node(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
    ) -> ast::Node {
        self.new_generated_name_for_node_ex(source, node, AutoGenerateOptions::default())
    }

    // Allocates a new unique name based on the provided node.
    pub fn new_generated_name_for_node_ex(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        mut options: AutoGenerateOptions,
    ) -> ast::Node {
        if !options.prefix.is_empty() || !options.suffix.is_empty() {
            options.flags |= GeneratedIdentifierFlags::OPTIMISTIC;
        }
        self.new_generated_identifier(
            GeneratedIdentifierFlags::NODE,
            "",
            Some((source, *node)),
            options,
        )
    }

    pub fn new_generated_name_for_node_with_prefix_and_suffix(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        flags: GeneratedIdentifierFlags,
        prefix: &str,
        suffix: &str,
    ) -> ast::Node {
        self.new_generated_identifier_with_prefix_and_suffix(
            GeneratedIdentifierFlags::NODE,
            "",
            Some((source, *node)),
            flags,
            prefix,
            suffix,
        )
    }

    pub fn new_generated_name_for_factory_node(&mut self, node: &ast::Node) -> ast::Node {
        self.new_generated_name_for_factory_node_ex(node, AutoGenerateOptions::default())
    }

    pub fn new_generated_name_for_factory_node_ex(
        &mut self,
        node: &ast::Node,
        mut options: AutoGenerateOptions,
    ) -> ast::Node {
        if !options.prefix.is_empty() || !options.suffix.is_empty() {
            options.flags |= GeneratedIdentifierFlags::OPTIMISTIC;
        }

        let id = super::emitcontext::NEXT_AUTO_GENERATE_ID
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        let mut text = if ast::is_member_name(self.node_factory.store(), *node) {
            self.node_factory.store().text(*node).to_string()
        } else {
            let generated_node = self.get_node_for_generated_name_worker(node, id);
            assert_eq!(
                generated_node.store_id(),
                self.node_factory.store().store_id(),
                "generated factory node must come from the current factory store"
            );
            format!(
                "(generated@{})",
                ast::get_node_id(self.node_factory.store(), generated_node)
            )
        };
        text = format_generated_name(false, options.prefix, &text, options.suffix);

        let name = self.node_factory.new_identifier(&text);
        let auto_generate = AutoGenerateInfo {
            id,
            flags: GeneratedIdentifierFlags::NODE
                | (options.flags & !GeneratedIdentifierFlags::KIND_MASK),
            prefix: options.prefix.to_string(),
            suffix: options.suffix.to_string(),
            node: *node,
        };
        self.state()
            .borrow_mut()
            .auto_generate
            .insert(node_key(&name), auto_generate);
        name
    }

    pub fn new_generated_name_for_factory_node_with_prefix_and_suffix(
        &mut self,
        node: &ast::Node,
        flags: GeneratedIdentifierFlags,
        prefix: &str,
        suffix: &str,
    ) -> ast::Node {
        let id = super::emitcontext::NEXT_AUTO_GENERATE_ID
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        let mut flags = flags;
        if !prefix.is_empty() || !suffix.is_empty() {
            flags |= GeneratedIdentifierFlags::OPTIMISTIC;
        }
        let mut text = if ast::is_member_name(self.node_factory.store(), *node) {
            self.node_factory.store().text(*node).to_string()
        } else {
            let generated_node = self.get_node_for_generated_name_worker(node, id);
            assert_eq!(
                generated_node.store_id(),
                self.node_factory.store().store_id(),
                "generated factory node must come from the current factory store"
            );
            format!(
                "(generated@{})",
                ast::get_node_id(self.node_factory.store(), generated_node)
            )
        };
        text = format_generated_name(false, prefix, &text, suffix);

        let name = self.node_factory.new_identifier(&text);
        let auto_generate = AutoGenerateInfo {
            id,
            flags: GeneratedIdentifierFlags::NODE | (flags & !GeneratedIdentifierFlags::KIND_MASK),
            prefix: prefix.to_string(),
            suffix: suffix.to_string(),
            node: *node,
        };
        self.state()
            .borrow_mut()
            .auto_generate
            .insert(node_key(&name), auto_generate);
        name
    }

    pub fn new_generated_private_name_for_factory_node_ex(
        &mut self,
        node: &ast::Node,
        mut options: AutoGenerateOptions,
    ) -> ast::Node {
        if !options.prefix.is_empty() || !options.suffix.is_empty() {
            options.flags |= GeneratedIdentifierFlags::OPTIMISTIC;
        }

        let id = super::emitcontext::NEXT_AUTO_GENERATE_ID
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        let mut text = if ast::is_member_name(self.node_factory.store(), *node) {
            self.node_factory.store().text(*node).to_string()
        } else {
            let generated_node = self.get_node_for_generated_name_worker(node, id);
            assert_eq!(
                generated_node.store_id(),
                self.node_factory.store().store_id(),
                "generated factory node must come from the current factory store"
            );
            format!(
                "(generated@{})",
                ast::get_node_id(self.node_factory.store(), generated_node)
            )
        };
        text = format_generated_name(true, options.prefix, &text, options.suffix);

        let name = self.node_factory.new_private_identifier(&text);
        let auto_generate = AutoGenerateInfo {
            id,
            flags: GeneratedIdentifierFlags::NODE
                | (options.flags & !GeneratedIdentifierFlags::KIND_MASK),
            prefix: options.prefix.to_string(),
            suffix: options.suffix.to_string(),
            node: *node,
        };
        self.state()
            .borrow_mut()
            .auto_generate
            .insert(node_key(&name), auto_generate);
        name
    }

    fn new_generated_private_identifier(
        &mut self,
        kind: GeneratedIdentifierFlags,
        text: &str,
        node: Option<(&ast::AstStore, ast::Node)>,
        options: AutoGenerateOptions,
    ) -> ast::Node {
        let id = super::emitcontext::NEXT_AUTO_GENERATE_ID
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        let mut text = text.to_string();

        if text.is_empty() {
            text = match node {
                None => format!("(auto@{id})"),
                Some((source, node)) => self.generated_name_text_for_node(source, node, id),
            };
            text = format_generated_name(true, options.prefix, &text, options.suffix);
        } else if !text.starts_with('#') {
            panic!("First character of private identifier must be #: {text}");
        }

        let name = self.node_factory.new_private_identifier(&text);
        let auto_generate = AutoGenerateInfo {
            id,
            flags: kind | (options.flags & !GeneratedIdentifierFlags::KIND_MASK),
            prefix: options.prefix.to_string(),
            suffix: options.suffix.to_string(),
            node: node.map(|(_, node)| node).unwrap_or(name),
        };
        self.state()
            .borrow_mut()
            .auto_generate
            .insert(node_key(&name), auto_generate);
        name
    }

    // Allocates a new unique private name based on the provided text.
    pub fn new_unique_private_name(&mut self, text: &str) -> ast::Node {
        self.new_unique_private_name_ex(text, AutoGenerateOptions::default())
    }

    // Allocates a new unique private name based on the provided text.
    pub fn new_unique_private_name_ex(
        &mut self,
        text: &str,
        options: AutoGenerateOptions,
    ) -> ast::Node {
        self.new_generated_private_identifier(GeneratedIdentifierFlags::UNIQUE, text, None, options)
    }

    // Allocates a new unique private name based on the provided node.
    pub fn new_generated_private_name_for_node(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
    ) -> ast::Node {
        self.new_generated_private_name_for_node_ex(source, node, AutoGenerateOptions::default())
    }

    // Allocates a new unique private name based on the provided node.
    pub fn new_generated_private_name_for_node_ex(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        mut options: AutoGenerateOptions,
    ) -> ast::Node {
        if !options.prefix.is_empty() || !options.suffix.is_empty() {
            options.flags |= GeneratedIdentifierFlags::OPTIMISTIC;
        }
        self.new_generated_private_identifier(
            GeneratedIdentifierFlags::NODE,
            "",
            Some((source, *node)),
            options,
        )
    }

    // Allocates a new StringLiteral whose source text is derived from the provided node. This is often used to create a
    // string representation of an Identifier or NumericLiteral.
    pub fn new_string_literal_from_node(
        &mut self,
        source: &ast::AstStore,
        text_source_node: &ast::Node,
    ) -> ast::Node {
        let mut text = String::new();
        match source.kind(*text_source_node) {
            ast::Kind::Identifier
            | ast::Kind::PrivateIdentifier
            | ast::Kind::JsxNamespacedName
            | ast::Kind::StringLiteral
            | ast::Kind::NumericLiteral
            | ast::Kind::BigIntLiteral
            | ast::Kind::NoSubstitutionTemplateLiteral
            | ast::Kind::TemplateHead
            | ast::Kind::TemplateMiddle
            | ast::Kind::TemplateTail
            | ast::Kind::RegularExpressionLiteral => {
                text = source.text(*text_source_node);
            }
            _ => {}
        }
        let node = self
            .node_factory
            .new_string_literal(&text, ast::TokenFlags::NONE);
        self.state()
            .borrow_mut()
            .text_source
            .insert(node_key(&node), *text_source_node);
        node
    }

    //
    // Common Tokens
    //

    pub fn new_this_expression(&mut self) -> ast::Node {
        self.node_factory
            .new_keyword_expression(ast::Kind::ThisKeyword)
    }

    pub fn new_true_expression(&mut self) -> ast::Node {
        self.node_factory
            .new_keyword_expression(ast::Kind::TrueKeyword)
    }

    pub fn new_false_expression(&mut self) -> ast::Node {
        self.node_factory
            .new_keyword_expression(ast::Kind::FalseKeyword)
    }

    //
    // Common Operators
    //

    pub fn new_comma_expression(&mut self, left: ast::Node, right: ast::Node) -> ast::Node {
        let operator = self.node_factory.new_token(ast::Kind::CommaToken);
        self.node_factory.new_binary_expression(
            None::<ast::ModifierList>,
            left,
            None::<ast::Node>,
            operator,
            right,
        )
    }

    pub fn new_assignment_expression(&mut self, left: ast::Node, right: ast::Node) -> ast::Node {
        let operator = self.node_factory.new_token(ast::Kind::EqualsToken);
        self.node_factory.new_binary_expression(
            None::<ast::ModifierList>,
            left,
            None::<ast::Node>,
            operator,
            right,
        )
    }

    pub fn new_logical_or_expression(&mut self, left: ast::Node, right: ast::Node) -> ast::Node {
        let operator = self.node_factory.new_token(ast::Kind::BarBarToken);
        self.node_factory.new_binary_expression(
            None::<ast::ModifierList>,
            left,
            None::<ast::Node>,
            operator,
            right,
        )
    }

    pub fn new_logical_and_expression(&mut self, left: ast::Node, right: ast::Node) -> ast::Node {
        let operator = self
            .node_factory
            .new_token(ast::Kind::AmpersandAmpersandToken);
        self.node_factory.new_binary_expression(
            None::<ast::ModifierList>,
            left,
            None::<ast::Node>,
            operator,
            right,
        )
    }

    // func (f *NodeFactory) NewLogicalANDExpression(left *ast.Expression, right *ast.Expression) *ast.Expression
    // func (f *NodeFactory) NewBitwiseORExpression(left *ast.Expression, right *ast.Expression) *ast.Expression
    // func (f *NodeFactory) NewBitwiseXORExpression(left *ast.Expression, right *ast.Expression) *ast.Expression
    // func (f *NodeFactory) NewBitwiseANDExpression(left *ast.Expression, right *ast.Expression) *ast.Expression
    pub fn new_strict_equality_expression(
        &mut self,
        left: ast::Node,
        right: ast::Node,
    ) -> ast::Node {
        let operator = self
            .node_factory
            .new_token(ast::Kind::EqualsEqualsEqualsToken);
        self.node_factory.new_binary_expression(
            None::<ast::ModifierList>,
            left,
            None::<ast::Node>,
            operator,
            right,
        )
    }

    pub fn new_strict_inequality_expression(
        &mut self,
        left: ast::Node,
        right: ast::Node,
    ) -> ast::Node {
        let operator = self
            .node_factory
            .new_token(ast::Kind::ExclamationEqualsEqualsToken);
        self.node_factory.new_binary_expression(
            None::<ast::ModifierList>,
            left,
            None::<ast::Node>,
            operator,
            right,
        )
    }

    //
    // Compound Nodes
    //

    pub fn new_void_zero_expression(&mut self) -> ast::Node {
        let zero = self
            .node_factory
            .new_numeric_literal("0", ast::TokenFlags::NONE);
        self.node_factory.new_void_expression(zero)
    }

    // Converts a slice of expressions into a single comma-delimited expression. Returns nil if expressions is nil or empty.
    pub fn inline_expressions(&mut self, expressions: &[ast::Node]) -> Option<ast::Node> {
        if expressions.is_empty() {
            return None;
        }
        if expressions.len() == 1 {
            return Some(expressions[0].clone());
        }
        // Avoid deeply nested comma expressions as traversing them during emit can result in "Maximum call
        // stack size exceeded" errors.
        let expressions = if expressions.len() > 10 {
            flatten_comma_elements(self.node_factory.store(), expressions)
        } else {
            expressions.to_vec()
        };
        let mut expression = expressions[0].clone();
        for next in &expressions[1..] {
            expression = self.new_comma_expression(expression, next.clone());
        }
        Some(expression)
    }

    //
    // Utilities
    //

    pub fn create_expression_from_entity_name(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
    ) -> ast::Node {
        if ast::is_qualified_name(source, *node) {
            let left = self.create_expression_from_entity_name(
                source,
                &source
                    .left(*node)
                    .expect("qualified name should have left node"),
            );
            let right_source = source
                .right(*node)
                .expect("qualified name should have right node");
            let right = self.clone_node_with_hooks(source, right_source);
            let right = self.copy_source_metadata(right, source, right_source);
            let prop_access = self.node_factory.new_property_access_expression(
                left,
                None::<ast::Node>,
                right,
                ast::NodeFlags::NONE,
            );
            return self.set_node_loc(prop_access, source.loc(*node));
        }
        let res = self.clone_node_with_hooks(source, *node);
        self.copy_source_metadata(res, source, *node)
    }

    pub fn restore_enclosing_label(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        outermost_labeled_statement: Option<&ast::Node>,
    ) -> ast::Node {
        let Some(outermost_labeled_statement) = outermost_labeled_statement else {
            return node.clone();
        };
        let statement = source.statement(*outermost_labeled_statement).unwrap();
        let inner_label = if ast::is_labeled_statement(source, statement) {
            self.restore_enclosing_label(source, node, Some(&statement))
        } else {
            *node
        };
        let label = source.label(*outermost_labeled_statement).unwrap();
        let label = if label.store_id() == self.node_factory.store().store_id() {
            label
        } else {
            let cloned = self.clone_node_with_hooks(source, label);
            self.copy_source_metadata(cloned, source, label)
        };
        self.node_factory.update_labeled_statement_from_store(
            source,
            *outermost_labeled_statement,
            label,
            inner_label,
        )
    }

    // CreateForOfBindingStatement creates a statement to bind the iteration value.
    pub fn create_for_of_binding_statement(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        bound_value: &ast::Node,
    ) -> ast::Node {
        if node.store_id() == self.node_factory.store().store_id() {
            if ast::is_variable_declaration_list(self.node_factory.store(), *node) {
                let (first_declaration, first_declaration_name, flags, loc) = {
                    let source = self.node_factory.store();
                    let first_declaration = source
                        .declarations(*node)
                        .expect("variable declaration list should have declarations")
                        .first()
                        .expect("variable declaration list should have first declaration");
                    (
                        first_declaration,
                        source.name(first_declaration).unwrap(),
                        source.flags(*node),
                        source.loc(*node),
                    )
                };
                let updated_declaration = self.node_factory.update_variable_declaration(
                    first_declaration,
                    first_declaration_name,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    Some(bound_value.clone()),
                );
                let declarations = self.new_node_list(vec![updated_declaration]);
                let updated_declaration_list =
                    self.node_factory
                        .update_variable_declaration_list(*node, declarations, flags);
                let statement = self
                    .node_factory
                    .new_variable_statement(None::<ast::ModifierList>, updated_declaration_list);
                return self.set_node_loc(statement, loc);
            }
            let loc = self.node_factory.store().loc(*node);
            let mut updated_expression =
                self.new_assignment_expression(node.clone(), bound_value.clone());
            updated_expression = self.set_node_loc(updated_expression, loc);
            let statement = self
                .node_factory
                .new_expression_statement(updated_expression);
            return self.set_node_loc(statement, loc);
        }

        {
            assert_eq!(
                node.store_id(),
                source.store_id(),
                "for-of binding initializer belongs to a different AST store"
            );
        }
        if ast::is_variable_declaration_list(source, *node) {
            let first_declaration = source
                .declarations(*node)
                .expect("variable declaration list should have declarations")
                .first()
                .expect("variable declaration list should have first declaration");
            let first_declaration_name = source.name(first_declaration).unwrap();
            let first_declaration_name = self.clone_node_with_hooks(source, first_declaration_name);
            let flags = source.flags(*node);
            let loc = source.loc(*node);
            let updated_declaration = self.node_factory.update_variable_declaration_from_store(
                source,
                first_declaration,
                first_declaration_name,
                None::<ast::Node>,
                None::<ast::Node>,
                Some(bound_value.clone()),
            );
            let declarations = self.new_node_list(vec![updated_declaration]);
            let updated_declaration_list = self
                .node_factory
                .update_variable_declaration_list_from_store(source, *node, declarations, flags);
            let statement = self
                .node_factory
                .new_variable_statement(None::<ast::ModifierList>, updated_declaration_list);
            return self.set_node_loc(statement, loc);
        }
        let loc = source.loc(*node);
        let target = self.clone_node_with_hooks(source, *node);
        let mut updated_expression = self.new_assignment_expression(target, bound_value.clone());
        updated_expression = self.set_node_loc(updated_expression, loc);
        let statement = self
            .node_factory
            .new_expression_statement(updated_expression);
        self.set_node_loc(statement, loc)
    }

    pub fn new_type_check(&mut self, value: &ast::Node, tag: &str) -> ast::Node {
        if tag == "null" {
            let null = self
                .node_factory
                .new_keyword_expression(ast::Kind::NullKeyword);
            self.new_strict_equality_expression(value.clone(), null)
        } else if tag == "undefined" {
            let void_zero = self.new_void_zero_expression();
            self.new_strict_equality_expression(value.clone(), void_zero)
        } else {
            let type_of = self.node_factory.new_type_of_expression(value.clone());
            let tag = self
                .node_factory
                .new_string_literal(tag, ast::TokenFlags::NONE);
            self.new_strict_equality_expression(type_of, tag)
        }
    }

    pub fn new_method_call(
        &mut self,
        object: &ast::Node,
        method_name: &ast::Node,
        arguments_list: &[ast::Node],
    ) -> ast::Node {
        let store = self.node_factory.store();
        let flags = if ast::is_call_expression(store, *object)
            && store
                .flags(*object)
                .contains(ast::NodeFlags::OPTIONAL_CHAIN)
        {
            ast::NodeFlags::OPTIONAL_CHAIN
        } else {
            ast::NodeFlags::NONE
        };
        let property_access = self.node_factory.new_property_access_expression(
            object.clone(),
            None::<ast::Node>,
            method_name.clone(),
            ast::NodeFlags::NONE,
        );
        let arguments = self.new_node_list(arguments_list.to_vec());
        self.node_factory.new_call_expression(
            property_access,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            flags,
        )
    }

    pub fn new_global_method_call(
        &mut self,
        global_object_name: &str,
        method_name: &str,
        arguments_list: &[ast::Node],
    ) -> ast::Node {
        let global_object = self.node_factory.new_identifier(global_object_name);
        let method = self.node_factory.new_identifier(method_name);
        self.new_method_call(&global_object, &method, arguments_list)
    }

    pub fn new_function_call_call(
        &mut self,
        target: &ast::Node,
        this_arg: Option<&ast::Node>,
        arguments_list: &[ast::Node],
    ) -> ast::Node {
        let Some(this_arg) = this_arg else {
            panic!("Attempted to construct function call call without this argument expression");
        };
        let mut args = vec![this_arg.clone()];
        args.extend_from_slice(arguments_list);
        let call = self.node_factory.new_identifier("call");
        self.new_method_call(target, &call, &args)
    }

    pub fn new_array_slice_call(&mut self, array: &ast::Node, start: i32) -> ast::Node {
        let mut args = Vec::new();
        if start != 0 {
            args.push(
                self.node_factory
                    .new_numeric_literal(&start.to_string(), ast::TokenFlags::NONE),
            );
        }
        let slice = self.node_factory.new_identifier("slice");
        self.new_method_call(array, &slice, &args)
    }

    // Determines whether a node is a parenthesized expression that can be ignored when recreating outer expressions.
    //
    // A parenthesized expression can be ignored when all of the following are true:
    //
    // - It's `pos` and `end` are not -1
    // - It does not have a custom source map range
    // - It does not have a custom comment range
    // - It does not have synthetic leading or trailing comments
    //
    // If an outermost parenthesized expression is ignored, but the containing expression requires a parentheses around
    // the expression to maintain precedence, a new parenthesized expression should be created automatically when
    // the containing expression is created/updated.
    pub fn is_ignorable_paren(&mut self, node: &ast::Node) -> bool {
        let store = self.node_factory.store();
        ast::is_parenthesized_expression(store, *node)
            && ast::node_is_synthesized(store, *node)
            && ast::range_is_synthesized(self.source_map_range(node))
            && ast::range_is_synthesized(self.comment_range(node))
        // && len(emitContext.SyntheticLeadingComments(node)) == 0 &&
        // len(emitContext.SyntheticTrailingComments(node)) == 0
    }

    pub fn restore_outer_expressions(
        &mut self,
        source: &ast::AstStore,
        outer_expression: Option<&ast::Node>,
        inner_expression: &ast::Node,
        kinds: ast::OuterExpressionKinds,
    ) -> ast::Node {
        if let Some(outer_expression) = outer_expression {
            if ast::is_outer_expression(source, *outer_expression, kinds)
                && !self.is_ignorable_paren(outer_expression)
            {
                let expression = self.restore_outer_expressions(
                    source,
                    source.expression(*outer_expression).as_ref(),
                    inner_expression,
                    ast::OuterExpressionKinds::ALL,
                );
                return self.update_outer_expression(source, outer_expression, expression);
            }
        }
        inner_expression.clone()
    }

    fn update_outer_expression(
        &mut self,
        source: &ast::AstStore,
        outer_expression: &ast::Node,
        expression: ast::Node,
    ) -> ast::Node {
        match source.kind(*outer_expression) {
            ast::Kind::ParenthesizedExpression => self
                .node_factory
                .update_parenthesized_expression(*outer_expression, expression),
            ast::Kind::TypeAssertionExpression => self.node_factory.update_type_assertion(
                *outer_expression,
                source.r#type(*outer_expression).unwrap(),
                expression,
            ),
            ast::Kind::AsExpression => self.node_factory.update_as_expression(
                *outer_expression,
                expression,
                source.r#type(*outer_expression).unwrap(),
            ),
            ast::Kind::SatisfiesExpression => self.node_factory.update_satisfies_expression(
                *outer_expression,
                expression,
                source.r#type(*outer_expression).unwrap(),
            ),
            ast::Kind::NonNullExpression => self.node_factory.update_non_null_expression(
                *outer_expression,
                expression,
                source.flags(*outer_expression),
            ),
            ast::Kind::ExpressionWithTypeArguments => {
                let mut importer = ast::AstImportState::new();
                let type_arguments = importer.preserve_optional_source_node_list(
                    &mut self.node_factory,
                    source.source_type_arguments(*outer_expression),
                );
                self.node_factory.update_expression_with_type_arguments(
                    *outer_expression,
                    expression,
                    type_arguments,
                )
            }
            ast::Kind::PartiallyEmittedExpression => self
                .node_factory
                .update_partially_emitted_expression(*outer_expression, expression),
            _ => panic!(
                "Unexpected outer expression kind: {:?}",
                source.kind(*outer_expression)
            ),
        }
    }

    // Ensures `"use strict"` is the first statement of a slice of statements.
    pub fn ensure_use_strict(
        &mut self,
        source: &ast::AstStore,
        statements: &[ast::Node],
    ) -> Vec<ast::Node> {
        for statement in statements {
            if ast::is_prologue_directive(source, *statement)
                && source
                    .expression(*statement)
                    .is_some_and(|expr| source.text(expr) == "use strict")
            {
                return statements.to_vec();
            } else {
                break;
            }
        }
        let use_strict_literal = self
            .node_factory
            .new_string_literal("use strict", ast::TokenFlags::NONE);
        let use_strict_prologue = self
            .node_factory
            .new_expression_statement(use_strict_literal);
        let mut result = vec![use_strict_prologue];
        result.extend_from_slice(statements);
        result
    }

    // Splits a slice of statements into two parts: standard prologue statements and the rest of the statements
    pub fn split_standard_prologue<'a>(
        &mut self,
        store: &ast::AstStore,
        source: &'a [ast::Node],
    ) -> (&'a [ast::Node], &'a [ast::Node]) {
        for (i, statement) in source.iter().enumerate() {
            if !ast::is_prologue_directive(store, *statement) {
                return (&source[..i], &source[i..]);
            }
        }
        (source, &[])
    }

    // Splits a slice of statements into two parts: custom prologue statements (e.g., with `EFCustomPrologue` set) and the rest of the statements
    pub fn split_custom_prologue<'a>(
        &mut self,
        store: &ast::AstStore,
        source: &'a [ast::Node],
    ) -> (&'a [ast::Node], &'a [ast::Node]) {
        for (i, statement) in source.iter().enumerate() {
            if ast::is_prologue_directive(store, *statement)
                || self.emit_flags(statement) & crate::EF_CUSTOM_PROLOGUE == 0
            {
                return (&source[..i], &source[i..]);
            }
        }
        (&[], source)
    }

    //
    // Declaration Names
    //

    fn get_name(
        &mut self,
        source: &ast::AstStore,
        node: Option<&ast::Node>,
        emit_flags: crate::EmitFlags,
        opts: AssignedNameOptions,
    ) -> ast::Node {
        let node_name = match node {
            Some(node) if opts.ignore_assigned_name => {
                ast::get_non_assigned_name_of_declaration(source, *node)
            }
            Some(node) => ast::get_name_of_declaration(source, Some(*node)),
            None => None,
        };

        if let Some(node_name) = node_name {
            let name = self.clone_node_with_hooks(source, node_name);
            let mut emit_flags = emit_flags;
            if !opts.allow_comments {
                emit_flags |= EF_NO_COMMENTS;
            }
            if !opts.allow_source_maps {
                emit_flags |= EF_NO_SOURCE_MAP;
            }
            self.mark_emit_node(&name, emit_flags);
            return name;
        }

        self.new_generated_name_for_node(
            source,
            node.expect("declaration node required when synthesizing name"),
        )
    }

    // Gets the local name of a declaration. This is primarily used for declarations that can be referred to by name in the
    // declaration's immediate scope (classes, enums, namespaces). A local name will *never* be prefixed with a module or
    // namespace export modifier like "exports." when emitted as an expression.
    pub fn get_local_name(&mut self, source: &ast::AstStore, node: &ast::Node) -> ast::Node {
        self.get_local_name_ex(source, node, AssignedNameOptions::default())
    }

    // Gets the local name of a declaration. This is primarily used for declarations that can be referred to by name in the
    // declaration's immediate scope (classes, enums, namespaces). A local name will *never* be prefixed with a module or
    // namespace export modifier like "exports." when emitted as an expression.
    pub fn get_local_name_ex(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        opts: AssignedNameOptions,
    ) -> ast::Node {
        self.get_name(source, Some(node), EF_LOCAL_NAME, opts)
    }

    pub fn get_local_name_of_factory_node(&mut self, node: &ast::Node) -> ast::Node {
        self.get_local_name_of_factory_node_ex(node, AssignedNameOptions::default())
    }

    pub fn get_local_name_of_factory_node_ex(
        &mut self,
        node: &ast::Node,
        opts: AssignedNameOptions,
    ) -> ast::Node {
        assert_eq!(node.store_id(), self.node_factory.store().store_id());
        let node_name = {
            let source = self.node_factory.store();
            ast::get_name_of_declaration(source, Some(*node))
        };

        if let Some(node_name) = node_name {
            let name = self
                .node_factory
                .deep_clone_node_in_current_store_preserve_location(node_name);
            let mut emit_flags = EF_LOCAL_NAME;
            if !opts.allow_comments {
                emit_flags |= EF_NO_COMMENTS;
            }
            if !opts.allow_source_maps {
                emit_flags |= EF_NO_SOURCE_MAP;
            }
            self.mark_emit_node(&name, emit_flags);
            return name;
        }

        let generated = self.new_generated_name_for_factory_node(node);
        self.mark_emit_node(&generated, EF_LOCAL_NAME);
        generated
    }

    // Gets the export name of a declaration. This is primarily used for declarations that can be
    // referred to by name in the declaration's immediate scope (classes, enums, namespaces). An
    // export name will *always* be prefixed with an module or namespace export modifier like
    // `"exports."` when emitted as an expression if the name points to an exported symbol.
    pub fn get_export_name(&mut self, source: &ast::AstStore, node: &ast::Node) -> ast::Node {
        self.get_export_name_ex(source, node, AssignedNameOptions::default())
    }

    // Gets the export name of a declaration. This is primarily used for declarations that can be
    // referred to by name in the declaration's immediate scope (classes, enums, namespaces). An
    // export name will *always* be prefixed with an module or namespace export modifier like
    // `"exports."` when emitted as an expression if the name points to an exported symbol.
    pub fn get_export_name_ex(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        opts: AssignedNameOptions,
    ) -> ast::Node {
        self.get_name(source, Some(node), EF_EXPORT_NAME, opts)
    }

    // Gets the name of a declaration to use during emit.
    pub fn get_declaration_name(&mut self, source: &ast::AstStore, node: &ast::Node) -> ast::Node {
        self.get_declaration_name_ex(source, node, NameOptions::default())
    }

    // Gets the name of a declaration to use during emit.
    pub fn get_declaration_name_ex(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        opts: NameOptions,
    ) -> ast::Node {
        self.get_name(
            source,
            Some(node),
            EF_NONE,
            AssignedNameOptions {
                allow_comments: opts.allow_comments,
                allow_source_maps: opts.allow_source_maps,
                ignore_assigned_name: false,
            },
        )
    }

    pub fn get_namespace_member_name(
        &mut self,
        source: &ast::AstStore,
        ns: &ast::Node,
        name: &ast::Node,
        opts: NameOptions,
    ) -> ast::Node {
        let name = if !self.has_auto_generate_info(Some(name)) {
            self.clone_node_with_hooks(source, *name)
        } else {
            name.clone()
        };
        let qualified_name = self.node_factory.new_property_access_expression(
            ns.clone(),
            None::<ast::Node>,
            name.clone(),
            ast::NodeFlags::NONE,
        );
        self.assign_comment_and_source_map_ranges(&qualified_name, &name);
        if !opts.allow_comments {
            self.mark_emit_node(&qualified_name, EF_NO_COMMENTS);
        }
        if !opts.allow_source_maps {
            self.mark_emit_node(&qualified_name, EF_NO_SOURCE_MAP);
        }
        qualified_name
    }

    // Gets the export name of a declaration for use in expressions.
    //
    // An export name will *always* be prefixed with a module or namespace export modifier like
    // `"exports."` when emitted as an expression if the name points to an exported symbol.
    pub fn get_external_module_or_namespace_export_name(
        &mut self,
        source: &ast::AstStore,
        ns: Option<&ast::Node>,
        node: &ast::Node,
        allow_comments: bool,
        allow_source_maps: bool,
    ) -> ast::Node {
        if let Some(ns) = ns {
            if ast::has_syntactic_modifier(source, *node, ast::ModifierFlags::EXPORT) {
                let name_opts = NameOptions {
                    allow_comments,
                    allow_source_maps,
                };
                let declaration_name = self.get_declaration_name_ex(source, node, name_opts);
                return self.get_namespace_member_name(source, ns, &declaration_name, name_opts);
            }
        }
        self.get_export_name_ex(
            source,
            node,
            AssignedNameOptions {
                allow_comments,
                allow_source_maps,
                ignore_assigned_name: false,
            },
        )
    }

    //
    // Emit Helpers
    //

    // Allocates a new Identifier representing a reference to a helper function.
    pub fn new_unscoped_helper_name(&mut self, name: &str) -> ast::Node {
        let node = self.node_factory.new_identifier(name);
        self.set_emit_flags(&node, EF_HELPER_NAME);
        node
    }

    // TypeScript Helpers

    pub fn new_decorate_helper(
        &mut self,
        decorator_expressions: &[ast::Node],
        target: ast::Node,
        member_name: Option<ast::Node>,
        descriptor: Option<ast::Node>,
    ) -> ast::Node {
        self.request_emit_helper(&DECORATE_HELPER);
        let decorator_list = self.new_node_list(decorator_expressions.to_vec());
        let decorators = self
            .node_factory
            .new_array_literal_expression(decorator_list, true);
        let mut arguments_array = vec![decorators, target];
        if let Some(member_name) = member_name {
            arguments_array.push(member_name);
            if let Some(descriptor) = descriptor {
                arguments_array.push(descriptor);
            }
        }
        let helper_name = self.new_unscoped_helper_name("__decorate");
        let arguments = self.new_node_list(arguments_array);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    pub fn new_metadata_helper(
        &mut self,
        metadata_key: &str,
        metadata_value: ast::Node,
    ) -> ast::Node {
        self.request_emit_helper(&METADATA_HELPER);
        let metadata_key = self
            .node_factory
            .new_string_literal(metadata_key, ast::TokenFlags::NONE);
        let helper_name = self.new_unscoped_helper_name("__metadata");
        let arguments = self.new_node_list(vec![metadata_key, metadata_value]);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    pub fn new_param_helper(
        &mut self,
        expression: ast::Node,
        parameter_offset: i32,
        location: core::TextRange,
    ) -> ast::Node {
        self.request_emit_helper(&PARAM_HELPER);
        let offset = self
            .node_factory
            .new_numeric_literal(&parameter_offset.to_string(), ast::TokenFlags::NONE);
        let helper_name = self.new_unscoped_helper_name("__param");
        let arguments = self.new_node_list(vec![offset, expression]);
        let helper = self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        );
        self.set_node_loc(helper, location)
    }

    // ESNext Helpers

    pub fn new_add_disposable_resource_helper(
        &mut self,
        env_binding: ast::Node,
        value: ast::Node,
        r#async: bool,
    ) -> ast::Node {
        self.request_emit_helper(&ADD_DISPOSABLE_RESOURCE_HELPER);
        let async_expr = self.node_factory.new_keyword_expression(if r#async {
            ast::Kind::TrueKeyword
        } else {
            ast::Kind::FalseKeyword
        });
        let helper_name = self.new_unscoped_helper_name("__addDisposableResource");
        let arguments = self.new_node_list(vec![env_binding, value, async_expr]);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    pub fn new_dispose_resources_helper(&mut self, env_binding: ast::Node) -> ast::Node {
        self.request_emit_helper(&DISPOSE_RESOURCES_HELPER);
        let helper_name = self.new_unscoped_helper_name("__disposeResources");
        let arguments = self.new_node_list(vec![env_binding]);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // Class Fields Helpers

    pub fn new_class_private_field_get_helper(
        &mut self,
        receiver: ast::Node,
        state: ast::Node,
        kind: PrivateIdentifierKind,
        fn_node: Option<ast::Node>,
    ) -> ast::Node {
        self.request_emit_helper(&CLASS_PRIVATE_FIELD_GET_HELPER);
        let mut args = vec![
            receiver,
            state,
            self.node_factory
                .new_string_literal(kind.as_str(), ast::TokenFlags::NONE),
        ];
        if let Some(fn_node) = fn_node {
            args.push(fn_node);
        }
        let helper_name = self.new_unscoped_helper_name("__classPrivateFieldGet");
        let arguments = self.new_node_list(args);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    pub fn new_class_private_field_set_helper(
        &mut self,
        receiver: ast::Node,
        state: ast::Node,
        value: ast::Node,
        kind: PrivateIdentifierKind,
        fn_node: Option<ast::Node>,
    ) -> ast::Node {
        self.request_emit_helper(&CLASS_PRIVATE_FIELD_SET_HELPER);
        let mut args = vec![
            receiver,
            state,
            value,
            self.node_factory
                .new_string_literal(kind.as_str(), ast::TokenFlags::NONE),
        ];
        if let Some(fn_node) = fn_node {
            args.push(fn_node);
        }
        let helper_name = self.new_unscoped_helper_name("__classPrivateFieldSet");
        let arguments = self.new_node_list(args);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    pub fn new_class_private_field_in_helper(
        &mut self,
        state: ast::Node,
        receiver: ast::Node,
    ) -> ast::Node {
        self.request_emit_helper(&CLASS_PRIVATE_FIELD_IN_HELPER);
        let helper_name = self.new_unscoped_helper_name("__classPrivateFieldIn");
        let arguments = self.new_node_list(vec![state, receiver]);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // Creates `Object.defineProperty(target, name, descriptor)`.
    pub fn new_object_define_property_call(
        &mut self,
        target: ast::Node,
        name: ast::Node,
        descriptor: ast::Node,
    ) -> ast::Node {
        let object = self.node_factory.new_identifier("Object");
        let define_property = self.node_factory.new_identifier("defineProperty");
        let callee = self.node_factory.new_property_access_expression(
            object,
            None::<ast::Node>,
            define_property,
            ast::NodeFlags::NONE,
        );
        let arguments = self.new_node_list(vec![target, name, descriptor]);
        self.node_factory.new_call_expression(
            callee,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // Creates `Reflect.get(target, propertyKey, receiver)`.
    pub fn new_reflect_get_call(
        &mut self,
        target: ast::Node,
        property_key: ast::Node,
        receiver: ast::Node,
    ) -> ast::Node {
        let reflect = self.node_factory.new_identifier("Reflect");
        let get = self.node_factory.new_identifier("get");
        let callee = self.node_factory.new_property_access_expression(
            reflect,
            None::<ast::Node>,
            get,
            ast::NodeFlags::NONE,
        );
        let arguments = self.new_node_list(vec![target, property_key, receiver]);
        self.node_factory.new_call_expression(
            callee,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // Creates `Reflect.set(target, propertyKey, value, receiver)`.
    pub fn new_reflect_set_call(
        &mut self,
        target: ast::Node,
        property_key: ast::Node,
        value: ast::Node,
        receiver: ast::Node,
    ) -> ast::Node {
        let reflect = self.node_factory.new_identifier("Reflect");
        let set = self.node_factory.new_identifier("set");
        let callee = self.node_factory.new_property_access_expression(
            reflect,
            None::<ast::Node>,
            set,
            ast::NodeFlags::NONE,
        );
        let arguments = self.new_node_list(vec![target, property_key, value, receiver]);
        self.node_factory.new_call_expression(
            callee,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // Creates `target.bind(thisArg, ...args)`.
    pub fn new_function_bind_call(
        &mut self,
        target: ast::Node,
        this_arg: ast::Node,
        arguments_list: &[ast::Node],
    ) -> ast::Node {
        let mut args = Vec::with_capacity(1 + arguments_list.len());
        args.push(this_arg);
        args.extend_from_slice(arguments_list);
        let bind = self.node_factory.new_identifier("bind");
        self.new_method_call(&target, &bind, &args)
    }

    // Creates `(() => { ...statements })()`.
    pub fn new_immediately_invoked_arrow_function(
        &mut self,
        statements: &[ast::Node],
    ) -> ast::Node {
        let parameters = self.new_node_list(vec![]);
        let equals_greater_than = self
            .node_factory
            .new_token(ast::Kind::EqualsGreaterThanToken);
        let statements = self.new_node_list(statements.to_vec());
        let body = self.node_factory.new_block(statements, true);
        let arrow = self.node_factory.new_arrow_function(
            None::<ast::ModifierList>,
            None::<ast::NodeList>,
            parameters,
            None::<ast::Node>,
            None::<ast::Node>,
            Some(equals_greater_than),
            body,
        );
        let paren = self.node_factory.new_parenthesized_expression(arrow);
        let arguments = self.new_node_list(vec![]);
        self.node_factory.new_call_expression(
            paren,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // Creates `export default <expression>;`.
    pub fn new_export_default(&mut self, expression: ast::Node) -> ast::Node {
        self.node_factory.new_export_assignment(
            None::<ast::ModifierList>,
            false,
            None::<ast::Node>,
            expression,
        )
    }

    // Creates `export { <name> };`.
    pub fn new_external_module_export(&mut self, name: ast::Node) -> ast::Node {
        let specifier = self
            .node_factory
            .new_export_specifier(false, None::<ast::Node>, name);
        let specifiers = self.new_node_list(vec![specifier]);
        let named_exports = self.node_factory.new_named_exports(specifiers);
        self.node_factory.new_export_declaration(
            None::<ast::ModifierList>,
            false,
            Some(named_exports),
            None::<ast::Node>,
            None::<ast::Node>,
        )
    }

    // ES2018 Helpers
    // Chains a sequence of expressions using the __assign helper or Object.assign if available in the target
    pub fn new_assign_helper(
        &mut self,
        attributes_segments: &[ast::Node],
        _script_target: core::ScriptTarget,
    ) -> ast::Node {
        let object = self.node_factory.new_identifier("Object");
        let assign = self.node_factory.new_identifier("assign");
        let callee = self.node_factory.new_property_access_expression(
            object,
            None::<ast::Node>,
            assign,
            ast::NodeFlags::NONE,
        );
        let arguments = self.new_node_list(attributes_segments.to_vec());
        self.node_factory.new_call_expression(
            callee,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // ES2018 Destructuring Helpers

    fn rest_helper_property_names(
        source: &ast::AstStore,
        elements: &[ast::Node],
        computed_temp_variables: Option<&[ast::Node]>,
    ) -> Vec<RestHelperPropertyName> {
        let mut property_names = Vec::new();
        let mut computed_temp_variable_offset = 0;
        for (i, element) in elements.iter().enumerate() {
            if i == elements.len() - 1 {
                break;
            }
            if let Some(property_name) =
                ast::try_get_property_name_of_binding_or_assignment_element(source, *element)
            {
                if ast::is_computed_property_name(source, property_name) {
                    let computed_temp_variables = computed_temp_variables.expect("Encountered computed property name but 'computedTempVariables' argument was not provided.");
                    let temp = computed_temp_variables[computed_temp_variable_offset].clone();
                    computed_temp_variable_offset += 1;
                    property_names.push(RestHelperPropertyName::Computed(temp));
                } else {
                    let text = match source.kind(property_name) {
                        ast::Kind::Identifier
                        | ast::Kind::PrivateIdentifier
                        | ast::Kind::JsxNamespacedName
                        | ast::Kind::StringLiteral
                        | ast::Kind::NumericLiteral
                        | ast::Kind::BigIntLiteral
                        | ast::Kind::NoSubstitutionTemplateLiteral
                        | ast::Kind::TemplateHead
                        | ast::Kind::TemplateMiddle
                        | ast::Kind::TemplateTail
                        | ast::Kind::RegularExpressionLiteral => source.text(property_name),
                        _ => String::new(),
                    };
                    property_names.push(RestHelperPropertyName::Literal {
                        text,
                        text_source_node: property_name,
                    });
                }
            }
        }
        property_names
    }

    pub fn new_rest_helper_current_store(
        &mut self,
        value: ast::Node,
        elements: &[ast::Node],
        computed_temp_variables: Option<&[ast::Node]>,
        location: core::TextRange,
    ) -> ast::Node {
        let property_names = Self::rest_helper_property_names(
            self.node_factory.store(),
            elements,
            computed_temp_variables,
        );
        self.new_rest_helper_from_property_names(value, property_names, location)
    }

    pub fn new_rest_helper(
        &mut self,
        source: &ast::AstStore,
        value: ast::Node,
        elements: &[ast::Node],
        computed_temp_variables: Option<&[ast::Node]>,
        location: core::TextRange,
    ) -> ast::Node {
        let property_names =
            Self::rest_helper_property_names(source, elements, computed_temp_variables);
        self.new_rest_helper_from_property_names(value, property_names, location)
    }

    fn new_rest_helper_from_property_names(
        &mut self,
        value: ast::Node,
        property_names: Vec<RestHelperPropertyName>,
        location: core::TextRange,
    ) -> ast::Node {
        self.request_emit_helper(&REST_HELPER);
        let mut property_name_nodes = Vec::new();
        for property_name in property_names {
            match property_name {
                RestHelperPropertyName::Computed(temp) => {
                    let type_check = self.new_type_check(&temp, "symbol");
                    let question_token = self.node_factory.new_token(ast::Kind::QuestionToken);
                    let colon_token = self.node_factory.new_token(ast::Kind::ColonToken);
                    let plus_token = self.node_factory.new_token(ast::Kind::PlusToken);
                    let empty_string = self
                        .node_factory
                        .new_string_literal("", ast::TokenFlags::NONE);
                    let stringified_temp = self.node_factory.new_binary_expression(
                        None::<ast::ModifierList>,
                        temp.clone(),
                        None::<ast::Node>,
                        plus_token,
                        empty_string,
                    );
                    property_name_nodes.push(self.node_factory.new_conditional_expression(
                        type_check,
                        question_token,
                        temp.clone(),
                        colon_token,
                        stringified_temp,
                    ));
                }
                RestHelperPropertyName::Literal {
                    text,
                    text_source_node,
                } => {
                    let node = self
                        .node_factory
                        .new_string_literal(&text, ast::TokenFlags::NONE);
                    self.state()
                        .borrow_mut()
                        .text_source
                        .insert(node_key(&node), text_source_node);
                    property_name_nodes.push(node);
                }
            }
        }
        let property_names = self.new_node_list(property_name_nodes);
        let prop_names = self
            .node_factory
            .new_array_literal_expression(property_names, false);
        let prop_names = self.set_node_loc(prop_names, location);
        let helper_name = self.new_unscoped_helper_name("__rest");
        let arguments = self.new_node_list(vec![value, prop_names]);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // ES2018 Helpers

    // Allocates a new Call expression to the `__await` helper.
    pub fn new_await_helper(&mut self, expression: ast::Node) -> ast::Node {
        self.request_emit_helper(&AWAIT_HELPER);
        let helper_name = self.new_unscoped_helper_name("__await");
        let arguments = self.new_node_list(vec![expression]);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // Allocates a new Call expression to the `__asyncGenerator` helper.
    pub fn new_async_generator_helper(
        &mut self,
        generator_func: ast::Node,
        has_lexical_this: bool,
    ) -> ast::Node {
        self.request_emit_helper(&AWAIT_HELPER);
        self.request_emit_helper(&ASYNC_GENERATOR_HELPER);
        self.mark_emit_node(
            &generator_func,
            EF_ASYNC_FUNCTION_BODY | EF_REUSE_TEMP_VARIABLE_SCOPE,
        );
        let this_arg = if has_lexical_this {
            self.new_this_expression()
        } else {
            self.new_void_zero_expression()
        };
        let helper_name = self.new_unscoped_helper_name("__asyncGenerator");
        let arguments_identifier = self.node_factory.new_identifier("arguments");
        let arguments = self.new_node_list(vec![this_arg, arguments_identifier, generator_func]);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // Allocates a new Call expression to the `__asyncDelegator` helper.
    pub fn new_async_delegator_helper(&mut self, expression: ast::Node) -> ast::Node {
        self.request_emit_helper(&AWAIT_HELPER);
        self.request_emit_helper(&ASYNC_DELEGATOR_HELPER);
        let helper_name = self.new_unscoped_helper_name("__asyncDelegator");
        let arguments = self.new_node_list(vec![expression]);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // Allocates a new Call expression to the `__asyncValues` helper.
    pub fn new_async_values_helper(&mut self, expression: ast::Node) -> ast::Node {
        self.request_emit_helper(&ASYNC_VALUES_HELPER);
        let helper_name = self.new_unscoped_helper_name("__asyncValues");
        let arguments = self.new_node_list(vec![expression]);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // !!! ES2017 Helpers

    // Allocates a new Call expression to the `__awaiter` helper.
    pub fn new_awaiter_helper(
        &mut self,
        has_lexical_this: bool,
        arguments_expression: Option<ast::Node>,
        parameters: Option<ast::NodeList>,
        body: ast::Node,
    ) -> ast::Node {
        self.request_emit_helper(&AWAITER_HELPER);
        let params = parameters.unwrap_or_else(|| self.new_node_list(vec![]));
        let asterisk = self.node_factory.new_token(ast::Kind::AsteriskToken);
        let generator_func = self.node_factory.new_function_expression(
            None::<ast::ModifierList>,
            Some(asterisk),
            None::<ast::Node>,
            None::<ast::NodeList>,
            params,
            None::<ast::Node>,
            None::<ast::Node>,
            body,
        );
        self.mark_emit_node(
            &generator_func,
            EF_ASYNC_FUNCTION_BODY | EF_REUSE_TEMP_VARIABLE_SCOPE,
        );
        let this_arg = if has_lexical_this {
            self.new_this_expression()
        } else {
            self.new_void_zero_expression()
        };
        let args_arg = arguments_expression.unwrap_or_else(|| self.new_void_zero_expression());
        let void_zero = self.new_void_zero_expression();
        let helper_name = self.new_unscoped_helper_name("__awaiter");
        let arguments = self.new_node_list(vec![this_arg, args_arg, void_zero, generator_func]);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // ES Decorator Helpers

    pub fn new_es_decorate_class_context_object(
        &mut self,
        name_expr: ast::Node,
        metadata: ast::Node,
    ) -> ast::Node {
        let kind_name = self.node_factory.new_identifier("kind");
        let class_value = self
            .node_factory
            .new_string_literal("class", ast::TokenFlags::NONE);
        let kind_prop = self.node_factory.new_property_assignment(
            None::<ast::ModifierList>,
            kind_name,
            None::<ast::Node>,
            None::<ast::Node>,
            class_value,
        );
        let name_name = self.node_factory.new_identifier("name");
        let name_prop = self.node_factory.new_property_assignment(
            None::<ast::ModifierList>,
            name_name,
            None::<ast::Node>,
            None::<ast::Node>,
            name_expr,
        );
        let metadata_name = self.node_factory.new_identifier("metadata");
        let metadata_prop = self.node_factory.new_property_assignment(
            None::<ast::ModifierList>,
            metadata_name,
            None::<ast::Node>,
            None::<ast::Node>,
            metadata,
        );
        let props = self.new_node_list(vec![kind_prop, name_prop, metadata_prop]);
        self.node_factory
            .new_object_literal_expression(props, false)
    }

    pub fn new_es_decorate_class_element_access_get_method(
        &mut self,
        name_computed: bool,
        name_expr: ast::Node,
    ) -> ast::Node {
        let accessor = if name_computed {
            let obj = self.node_factory.new_identifier("obj");
            self.node_factory.new_element_access_expression(
                obj,
                None::<ast::Node>,
                name_expr,
                ast::NodeFlags::NONE,
            )
        } else {
            let obj = self.node_factory.new_identifier("obj");
            self.node_factory.new_property_access_expression(
                obj,
                None::<ast::Node>,
                name_expr,
                ast::NodeFlags::NONE,
            )
        };
        let obj_name = self.node_factory.new_identifier("obj");
        let obj_param = self.node_factory.new_parameter_declaration(
            None::<ast::ModifierList>,
            None::<ast::Node>,
            obj_name,
            None::<ast::Node>,
            None::<ast::Node>,
            None::<ast::Node>,
        );
        let parameters = self.new_node_list(vec![obj_param]);
        let equals_greater_than = self
            .node_factory
            .new_token(ast::Kind::EqualsGreaterThanToken);
        let arrow = self.node_factory.new_arrow_function(
            None::<ast::ModifierList>,
            None::<ast::NodeList>,
            parameters,
            None::<ast::Node>,
            None::<ast::Node>,
            Some(equals_greater_than),
            accessor,
        );
        let get_name = self.node_factory.new_identifier("get");
        self.node_factory.new_property_assignment(
            None::<ast::ModifierList>,
            get_name,
            None::<ast::Node>,
            None::<ast::Node>,
            arrow,
        )
    }

    pub fn new_es_decorate_class_element_access_set_method(
        &mut self,
        name_computed: bool,
        name_expr: ast::Node,
    ) -> ast::Node {
        let accessor = if name_computed {
            let obj = self.node_factory.new_identifier("obj");
            self.node_factory.new_element_access_expression(
                obj,
                None::<ast::Node>,
                name_expr,
                ast::NodeFlags::NONE,
            )
        } else {
            let obj = self.node_factory.new_identifier("obj");
            self.node_factory.new_property_access_expression(
                obj,
                None::<ast::Node>,
                name_expr,
                ast::NodeFlags::NONE,
            )
        };
        let value = self.node_factory.new_identifier("value");
        let assignment = self.new_assignment_expression(accessor, value);
        let stmt = self.node_factory.new_expression_statement(assignment);
        let statements = self.new_node_list(vec![stmt]);
        let body = self.node_factory.new_block(statements, false);
        let obj_name = self.node_factory.new_identifier("obj");
        let obj_param = self.node_factory.new_parameter_declaration(
            None::<ast::ModifierList>,
            None::<ast::Node>,
            obj_name,
            None::<ast::Node>,
            None::<ast::Node>,
            None::<ast::Node>,
        );
        let value_name = self.node_factory.new_identifier("value");
        let value_param = self.node_factory.new_parameter_declaration(
            None::<ast::ModifierList>,
            None::<ast::Node>,
            value_name,
            None::<ast::Node>,
            None::<ast::Node>,
            None::<ast::Node>,
        );
        let parameters = self.new_node_list(vec![obj_param, value_param]);
        let equals_greater_than = self
            .node_factory
            .new_token(ast::Kind::EqualsGreaterThanToken);
        let arrow = self.node_factory.new_arrow_function(
            None::<ast::ModifierList>,
            None::<ast::NodeList>,
            parameters,
            None::<ast::Node>,
            None::<ast::Node>,
            Some(equals_greater_than),
            body,
        );
        let set_name = self.node_factory.new_identifier("set");
        self.node_factory.new_property_assignment(
            None::<ast::ModifierList>,
            set_name,
            None::<ast::Node>,
            None::<ast::Node>,
            arrow,
        )
    }

    pub fn new_es_decorate_class_element_access_has_method(
        &mut self,
        _source: &ast::AstStore,
        name_computed: bool,
        name_expr: ast::Node,
    ) -> ast::Node {
        let property_name =
            if !name_computed && ast::is_identifier(self.node_factory.store(), name_expr) {
                let text = self.node_factory.store().text(name_expr);
                self.node_factory
                    .new_string_literal(&text, ast::TokenFlags::NONE)
            } else {
                name_expr
            };
        let obj_name = self.node_factory.new_identifier("obj");
        let obj_param = self.node_factory.new_parameter_declaration(
            None::<ast::ModifierList>,
            None::<ast::Node>,
            obj_name,
            None::<ast::Node>,
            None::<ast::Node>,
            None::<ast::Node>,
        );
        let in_token = self.node_factory.new_token(ast::Kind::InKeyword);
        let obj = self.node_factory.new_identifier("obj");
        let in_expr = self.node_factory.new_binary_expression(
            None::<ast::ModifierList>,
            property_name,
            None::<ast::Node>,
            in_token,
            obj,
        );
        let parameters = self.new_node_list(vec![obj_param]);
        let equals_greater_than = self
            .node_factory
            .new_token(ast::Kind::EqualsGreaterThanToken);
        let arrow = self.node_factory.new_arrow_function(
            None::<ast::ModifierList>,
            None::<ast::NodeList>,
            parameters,
            None::<ast::Node>,
            None::<ast::Node>,
            Some(equals_greater_than),
            in_expr,
        );
        let has_name = self.node_factory.new_identifier("has");
        self.node_factory.new_property_assignment(
            None::<ast::ModifierList>,
            has_name,
            None::<ast::Node>,
            None::<ast::Node>,
            arrow,
        )
    }

    // Creates the "access" object for a class element decorator context.
    pub fn new_es_decorate_class_element_access_object(
        &mut self,
        source: &ast::AstStore,
        name_computed: bool,
        name_expr: ast::Node,
        has_get: bool,
        has_set: bool,
    ) -> ast::Node {
        let mut access_props = vec![self.new_es_decorate_class_element_access_has_method(
            source,
            name_computed,
            name_expr.clone(),
        )];
        if has_get {
            access_props.push(
                self.new_es_decorate_class_element_access_get_method(
                    name_computed,
                    name_expr.clone(),
                ),
            );
        }
        if has_set {
            access_props.push(
                self.new_es_decorate_class_element_access_set_method(name_computed, name_expr),
            );
        }
        let access_props = self.new_node_list(access_props);
        self.node_factory
            .new_object_literal_expression(access_props, false)
    }

    pub fn new_es_decorate_class_element_context_object(
        &mut self,
        source: &ast::AstStore,
        kind: &str,
        name_computed: bool,
        name_expr: ast::Node,
        is_static: bool,
        is_private: bool,
        has_get: bool,
        has_set: bool,
        metadata: ast::Node,
    ) -> ast::Node {
        let name_value = if !name_computed
            && (ast::is_private_identifier(self.node_factory.store(), name_expr)
                || ast::is_identifier(self.node_factory.store(), name_expr))
        {
            let text = self.node_factory.store().text(name_expr);
            self.node_factory
                .new_string_literal(&text, ast::TokenFlags::NONE)
        } else {
            name_expr.clone()
        };
        let access_obj = self.new_es_decorate_class_element_access_object(
            source,
            name_computed,
            name_expr,
            has_get,
            has_set,
        );
        let static_expr = if is_static {
            self.new_true_expression()
        } else {
            self.new_false_expression()
        };
        let private_expr = if is_private {
            self.new_true_expression()
        } else {
            self.new_false_expression()
        };
        let kind_name = self.node_factory.new_identifier("kind");
        let kind_value = self
            .node_factory
            .new_string_literal(kind, ast::TokenFlags::NONE);
        let kind_prop = self.node_factory.new_property_assignment(
            None::<ast::ModifierList>,
            kind_name,
            None::<ast::Node>,
            None::<ast::Node>,
            kind_value,
        );
        let name_name = self.node_factory.new_identifier("name");
        let name_prop = self.node_factory.new_property_assignment(
            None::<ast::ModifierList>,
            name_name,
            None::<ast::Node>,
            None::<ast::Node>,
            name_value,
        );
        let static_name = self.node_factory.new_identifier("static");
        let static_prop = self.node_factory.new_property_assignment(
            None::<ast::ModifierList>,
            static_name,
            None::<ast::Node>,
            None::<ast::Node>,
            static_expr,
        );
        let private_name = self.node_factory.new_identifier("private");
        let private_prop = self.node_factory.new_property_assignment(
            None::<ast::ModifierList>,
            private_name,
            None::<ast::Node>,
            None::<ast::Node>,
            private_expr,
        );
        let access_name = self.node_factory.new_identifier("access");
        let access_prop = self.node_factory.new_property_assignment(
            None::<ast::ModifierList>,
            access_name,
            None::<ast::Node>,
            None::<ast::Node>,
            access_obj,
        );
        let metadata_name = self.node_factory.new_identifier("metadata");
        let metadata_prop = self.node_factory.new_property_assignment(
            None::<ast::ModifierList>,
            metadata_name,
            None::<ast::Node>,
            None::<ast::Node>,
            metadata,
        );
        let props = self.new_node_list(vec![
            kind_prop,
            name_prop,
            static_prop,
            private_prop,
            access_prop,
            metadata_prop,
        ]);
        self.node_factory
            .new_object_literal_expression(props, false)
    }

    pub fn new_es_decorate_helper(
        &mut self,
        ctor: ast::Node,
        descriptor_in: ast::Node,
        decorators: ast::Node,
        context_in: ast::Node,
        initializers: ast::Node,
        extra_initializers: ast::Node,
    ) -> ast::Node {
        self.request_emit_helper(&ES_DECORATE_HELPER);
        let helper_name = self.new_unscoped_helper_name("__esDecorate");
        let arguments = self.new_node_list(vec![
            ctor,
            descriptor_in,
            decorators,
            context_in,
            initializers,
            extra_initializers,
        ]);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    pub fn new_run_initializers_helper(
        &mut self,
        this_arg: ast::Node,
        initializers: ast::Node,
        value: Option<ast::Node>,
    ) -> ast::Node {
        self.request_emit_helper(&RUN_INITIALIZERS_HELPER);
        let mut arguments = vec![this_arg, initializers];
        if let Some(value) = value {
            arguments.push(value);
        }
        let helper_name = self.new_unscoped_helper_name("__runInitializers");
        let arguments = self.new_node_list(arguments);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // ES2015 Helpers

    pub fn new_template_object_helper(
        &mut self,
        cooked_array: ast::Node,
        raw_array: ast::Node,
    ) -> ast::Node {
        self.request_emit_helper(&MAKE_TEMPLATE_OBJECT_HELPER);
        let helper_name = self.new_unscoped_helper_name("__makeTemplateObject");
        let arguments = self.new_node_list(vec![cooked_array, raw_array]);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    pub fn new_prop_key_helper(&mut self, expr: ast::Node) -> ast::Node {
        self.request_emit_helper(&PROP_KEY_HELPER);
        let helper_name = self.new_unscoped_helper_name("__propKey");
        let arguments = self.new_node_list(vec![expr]);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    pub fn new_set_function_name_helper(
        &mut self,
        fn_node: ast::Node,
        name: ast::Node,
        prefix: &str,
    ) -> ast::Node {
        self.request_emit_helper(&SET_FUNCTION_NAME_HELPER);
        let mut arguments = vec![fn_node, name];
        if !prefix.is_empty() {
            arguments.push(
                self.node_factory
                    .new_string_literal(prefix, ast::TokenFlags::NONE),
            );
        }
        let helper_name = self.new_unscoped_helper_name("__setFunctionName");
        let arguments = self.new_node_list(arguments);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // ES Module Helpers

    // Allocates a new Call expression to the `__importDefault` helper.
    pub fn new_import_default_helper(&mut self, expression: ast::Node) -> ast::Node {
        self.request_emit_helper(&IMPORT_DEFAULT_HELPER);
        let helper_name = self.new_unscoped_helper_name("__importDefault");
        let arguments = self.new_node_list(vec![expression]);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // Allocates a new Call expression to the `__importStar` helper.
    pub fn new_import_star_helper(&mut self, expression: ast::Node) -> ast::Node {
        self.request_emit_helper(&IMPORT_STAR_HELPER);
        let helper_name = self.new_unscoped_helper_name("__importStar");
        let arguments = self.new_node_list(vec![expression]);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    // Allocates a new Call expression to the `__exportStar` helper.
    pub fn new_export_star_helper(
        &mut self,
        module_expression: ast::Node,
        exports_expression: ast::Node,
    ) -> ast::Node {
        self.request_emit_helper(&EXPORT_STAR_HELPER);
        let helper_name = self.new_unscoped_helper_name("__exportStar");
        let arguments = self.new_node_list(vec![module_expression, exports_expression]);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }

    pub fn new_assignment_target_wrapper(
        &mut self,
        param_name: ast::Node,
        expression: ast::Node,
    ) -> ast::Node {
        let value_name = self.node_factory.new_identifier("value");
        let parameter = self.node_factory.new_parameter_declaration(
            None::<ast::ModifierList>,
            None::<ast::Node>,
            param_name,
            None::<ast::Node>,
            None::<ast::Node>,
            None::<ast::Node>,
        );
        let parameters = self.new_node_list(vec![parameter]);
        let expression_statement = self.node_factory.new_expression_statement(expression);
        let statements = self.new_node_list(vec![expression_statement]);
        let body = self.node_factory.new_block(statements, false);
        let set_accessor = self.node_factory.new_set_accessor_declaration(
            None::<ast::ModifierList>,
            value_name,
            None::<ast::NodeList>,
            parameters,
            None::<ast::Node>,
            None::<ast::Node>,
            Some(body),
        );
        let properties = self.new_node_list(vec![set_accessor]);
        let obj_literal = self
            .node_factory
            .new_object_literal_expression(properties, false);
        let paren = self.node_factory.new_parenthesized_expression(obj_literal);
        let value = self.node_factory.new_identifier("value");
        self.node_factory.new_property_access_expression(
            paren,
            None::<ast::Node>,
            value,
            ast::NodeFlags::NONE,
        )
    }

    // Allocates a new Call expression to the `__rewriteRelativeImportExtension` helper.
    pub fn new_rewrite_relative_import_extensions_helper(
        &mut self,
        first_argument: ast::Node,
        preserve_jsx: bool,
    ) -> ast::Node {
        self.request_emit_helper(&REWRITE_RELATIVE_IMPORT_EXTENSIONS_HELPER);
        let mut arguments = vec![first_argument];
        if preserve_jsx {
            arguments.push(self.node_factory.new_token(ast::Kind::TrueKeyword));
        }
        let helper_name = self.new_unscoped_helper_name("__rewriteRelativeImportExtension");
        let arguments = self.new_node_list(arguments);
        self.node_factory.new_call_expression(
            helper_name,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        )
    }
}

fn flatten_comma_element(
    source: &ast::AstStore,
    node: &ast::Node,
    mut expressions: Vec<ast::Node>,
) -> Vec<ast::Node> {
    if ast::is_binary_expression(source, *node)
        && ast::node_is_synthesized(source, *node)
        && source
            .operator_token(*node)
            .is_some_and(|operator| source.kind(operator) == ast::Kind::CommaToken)
    {
        let left = source
            .left(*node)
            .expect("binary expression should have left");
        let right = source
            .right(*node)
            .expect("binary expression should have right");
        expressions = flatten_comma_element(source, &left, expressions);
        expressions = flatten_comma_element(source, &right, expressions);
    } else {
        expressions.push(node.clone());
    }
    expressions
}

fn flatten_comma_elements(source: &ast::AstStore, expressions: &[ast::Node]) -> Vec<ast::Node> {
    let mut result = Vec::new();
    for expression in expressions {
        result = flatten_comma_element(source, expression, result);
    }
    result
}

#[derive(Clone, Copy, Default)]
pub struct NameOptions {
    pub allow_comments: bool, // indicates whether comments may be emitted for the name.
    pub allow_source_maps: bool, // indicates whether source maps may be emitted for the name.
}

#[derive(Clone, Copy, Default)]
pub struct AssignedNameOptions {
    pub allow_comments: bool, // indicates whether comments may be emitted for the name.
    pub allow_source_maps: bool, // indicates whether source maps may be emitted for the name.
    pub ignore_assigned_name: bool, // indicates whether the assigned name of a declaration shouldn't be considered.
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivateIdentifierKind {
    Field,
    Method,
    Accessor,
    Untransformed,
}

impl PrivateIdentifierKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PrivateIdentifierKind::Field => "f",
            PrivateIdentifierKind::Method => "m",
            PrivateIdentifierKind::Accessor => "a",
            PrivateIdentifierKind::Untransformed => "untransformed",
        }
    }
}

fn node_key(node: &ast::Node) -> ast::Node {
    *node
}

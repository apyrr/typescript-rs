use ts_ast as ast;
use ts_ast::SymbolFlagsExt;
use ts_core as core;
use ts_diagnostics as diagnostics;

use crate::ProgramBindingState;

pub struct ReferenceResolverHooks<'a> {
    pub resolve_name: Option<
        Box<
            dyn Fn(
                    ast::Node,
                    &str,
                    ast::SymbolFlags,
                    Option<&'static diagnostics::Message>,
                    bool,
                    bool,
                ) -> Option<ast::SymbolHandle>
                + 'a,
        >,
    >,
    pub get_resolved_symbol: Option<Box<dyn Fn(ast::Node) -> Option<ast::SymbolHandle> + 'a>>,
    pub get_merged_symbol: Option<Box<dyn Fn(ast::SymbolHandle) -> Option<ast::SymbolHandle> + 'a>>,
    pub get_parent_of_symbol:
        Option<Box<dyn Fn(ast::SymbolHandle) -> Option<ast::SymbolHandle> + 'a>>,
    pub get_symbol_of_declaration: Option<Box<dyn Fn(ast::Node) -> Option<ast::SymbolHandle> + 'a>>,
    pub get_type_only_alias_declaration:
        Option<Box<dyn Fn(ast::SymbolHandle, ast::SymbolFlags) -> Option<ast::Declaration> + 'a>>,
    pub get_export_symbol_of_value_symbol_if_exported:
        Option<Box<dyn Fn(ast::SymbolHandle) -> Option<ast::SymbolHandle> + 'a>>,
    pub get_element_access_expression_name: Option<Box<dyn Fn(ast::Node) -> (String, bool) + 'a>>,
}

impl Default for ReferenceResolverHooks<'_> {
    fn default() -> Self {
        Self {
            resolve_name: None,
            get_resolved_symbol: None,
            get_merged_symbol: None,
            get_parent_of_symbol: None,
            get_symbol_of_declaration: None,
            get_type_only_alias_declaration: None,
            get_export_symbol_of_value_symbol_if_exported: None,
            get_element_access_expression_name: None,
        }
    }
}

pub struct ReferenceResolver<'a> {
    store: &'a ast::AstStore,
    binding_state: Option<&'a ProgramBindingState>,
    _options: core::CompilerOptions,
    hooks: ReferenceResolverHooks<'a>,
}

pub trait BinderReferenceResolver {
    fn get_referenced_export_container(
        &mut self,
        node: ast::IdentifierNode,
        prefix_locals: bool,
    ) -> Option<ast::Node>;
    fn get_referenced_import_declaration(
        &mut self,
        node: ast::IdentifierNode,
    ) -> Option<ast::Declaration>;
    fn get_referenced_value_declaration(
        &mut self,
        node: ast::IdentifierNode,
    ) -> Option<ast::Declaration>;
    fn get_referenced_value_declarations(
        &mut self,
        node: ast::IdentifierNode,
    ) -> Vec<ast::Declaration>;
    fn get_element_access_expression_name(&mut self, expression: ast::Node) -> String;
    fn get_referenced_member_value_declaration(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::Declaration>;
}

pub fn new_reference_resolver<'a>(
    store: &'a ast::AstStore,
    options: core::CompilerOptions,
    hooks: ReferenceResolverHooks<'a>,
) -> ReferenceResolver<'a> {
    ReferenceResolver {
        store,
        binding_state: None,
        _options: options,
        hooks,
    }
}

pub fn new_bound_reference_resolver<'a>(
    store: &'a ast::AstStore,
    binding_state: &'a ProgramBindingState,
    options: core::CompilerOptions,
    hooks: ReferenceResolverHooks<'a>,
) -> ReferenceResolver<'a> {
    ReferenceResolver {
        store,
        binding_state: Some(binding_state),
        _options: options,
        hooks,
    }
}

impl<'a> ReferenceResolver<'a> {
    fn symbol_flags(&self, symbol: ast::SymbolHandle) -> Option<ast::SymbolFlags> {
        self.binding_state
            .map(|binding_state| binding_state.symbol_flags(symbol))
    }

    fn with_symbol_declarations<R>(
        &self,
        symbol: ast::SymbolHandle,
        f: impl FnOnce(&[ast::Node]) -> R,
    ) -> Option<R> {
        self.binding_state
            .map(|binding_state| binding_state.with_symbol_declarations(symbol, f))
    }

    fn symbol_value_declaration(&self, symbol: ast::SymbolHandle) -> Option<ast::Node> {
        self.binding_state
            .and_then(|binding_state| binding_state.symbol_value_declaration(symbol))
    }

    fn symbol_parent(&self, symbol: ast::SymbolHandle) -> Option<ast::SymbolHandle> {
        self.binding_state
            .and_then(|binding_state| binding_state.symbol_parent(symbol))
    }

    fn symbol_export_symbol(&self, symbol: ast::SymbolHandle) -> Option<ast::SymbolHandle> {
        self.binding_state
            .and_then(|binding_state| binding_state.symbol_export_symbol(symbol))
    }

    fn get_resolved_symbol(&self, node: Option<ast::Node>) -> Option<ast::SymbolHandle> {
        let node = node?;
        self.hooks
            .get_resolved_symbol
            .as_ref()
            .and_then(|hook| hook(node))
    }

    fn get_merged_symbol(&self, symbol: Option<ast::SymbolHandle>) -> Option<ast::SymbolHandle> {
        let symbol = symbol?;
        if let Some(hook) = self.hooks.get_merged_symbol.as_ref() {
            return hook(symbol);
        }
        Some(symbol)
    }

    fn get_parent_of_symbol(&self, symbol: Option<ast::SymbolHandle>) -> Option<ast::SymbolHandle> {
        let symbol = symbol?;
        if let Some(hook) = self.hooks.get_parent_of_symbol.as_ref() {
            return hook(symbol);
        }
        self.symbol_parent(symbol)
    }

    fn get_symbol_of_declaration(
        &self,
        declaration: Option<ast::Node>,
    ) -> Option<ast::SymbolHandle> {
        let declaration = declaration?;
        if let Some(hook) = self.hooks.get_symbol_of_declaration.as_ref() {
            return hook(declaration);
        }
        self.binding_state
            .and_then(|state| state.symbol(declaration))
    }

    fn get_referenced_value_symbol(
        &mut self,
        reference: ast::IdentifierNode,
        start_in_declaration_container: bool,
    ) -> Option<ast::SymbolHandle> {
        if let Some(resolved_symbol) = self.get_resolved_symbol(Some(reference)) {
            return Some(resolved_symbol);
        }

        let reference_node = reference;
        let mut location = reference_node;
        let reference_parent = self.store.parent(reference_node);
        if start_in_declaration_container
            && reference_parent.as_ref().is_some_and(|parent| {
                ast::is_declaration(self.store, *parent)
                    && self.store.name(*parent) == Some(reference_node)
            })
        {
            if let Some(container) =
                ast::get_declaration_container(self.store, *reference_parent.as_ref().unwrap())
            {
                location = container;
            }
        }

        if let Some(hook) = self.hooks.resolve_name.as_ref() {
            return hook(
                location,
                &self.store.text(reference_node),
                ast::SYMBOL_FLAGS_EXPORT_VALUE | ast::SYMBOL_FLAGS_VALUE | ast::SYMBOL_FLAGS_ALIAS,
                None,  /* nameNotFoundMessage */
                false, /* isUse */
                false, /* excludeGlobals */
            );
        }

        None
    }

    fn is_type_only_alias_declaration(&self, symbol: Option<ast::SymbolHandle>) -> bool {
        let Some(symbol) = symbol else {
            return false;
        };
        if let Some(hook) = self.hooks.get_type_only_alias_declaration.as_ref() {
            return hook(symbol, ast::SYMBOL_FLAGS_VALUE).is_some();
        }

        let mut node = self.get_declaration_of_alias_symbol(symbol);
        while let Some(current) = node {
            match self.store.kind(current) {
                ast::Kind::ImportEqualsDeclaration | ast::Kind::ExportDeclaration => {
                    return self.store.is_type_only(current).unwrap_or(false);
                }
                ast::Kind::ImportClause
                | ast::Kind::ImportSpecifier
                | ast::Kind::ExportSpecifier => {
                    if self.store.is_type_only(current).unwrap_or(false) {
                        return true;
                    }
                    node = self.store.parent(current);
                    continue;
                }
                ast::Kind::NamedImports | ast::Kind::NamedExports => {
                    node = self.store.parent(current);
                    continue;
                }
                _ => break,
            }
        }
        false
    }

    fn get_declaration_of_alias_symbol(&self, symbol: ast::SymbolHandle) -> Option<ast::Node> {
        self.with_symbol_declarations(symbol, |declarations| {
            declarations
                .iter()
                .rev()
                .copied()
                .find(|declaration| ast::is_alias_symbol_declaration(self.store, *declaration))
        })?
    }

    fn is_non_local_alias(&self, symbol: ast::SymbolHandle, excludes: ast::SymbolFlags) -> bool {
        let Some(flags) = self.symbol_flags(symbol) else {
            return false;
        };
        flags & (ast::SYMBOL_FLAGS_ALIAS | excludes) == ast::SYMBOL_FLAGS_ALIAS
            || flags & ast::SYMBOL_FLAGS_ALIAS != ast::SYMBOL_FLAGS_NONE
                && flags & ast::SYMBOL_FLAGS_ASSIGNMENT != ast::SYMBOL_FLAGS_NONE
    }

    fn get_export_symbol_of_value_symbol_if_exported(
        &self,
        symbol: Option<ast::SymbolHandle>,
    ) -> Option<ast::SymbolHandle> {
        let symbol = symbol?;
        if let Some(hook) = self
            .hooks
            .get_export_symbol_of_value_symbol_if_exported
            .as_ref()
        {
            return hook(symbol);
        }

        let exported = if self
            .symbol_flags(symbol)?
            .intersects(ast::SYMBOL_FLAGS_EXPORT_VALUE)
        {
            self.symbol_export_symbol(symbol).unwrap_or(symbol)
        } else {
            symbol
        };
        self.get_merged_symbol(Some(exported))
    }

    pub fn get_referenced_export_container(
        &mut self,
        node: ast::IdentifierNode,
        prefix_locals: bool,
    ) -> Option<ast::Node> {
        // When resolving the export for the name of a module or enum
        // declaration, we need to start resolution at the declaration's container.
        // Otherwise, we could incorrectly resolve the export as the
        // declaration if it contains an exported member with the same name.
        let node_handle = node;
        let start_in_declaration_container = self.store.parent(node_handle).is_some_and(|parent| {
            (self.store.kind(parent) == ast::Kind::ModuleDeclaration
                || self.store.kind(parent) == ast::Kind::EnumDeclaration)
                && self.store.name(parent) == Some(node_handle)
        });

        let mut symbol = self.get_referenced_value_symbol(node, start_in_declaration_container)?;
        if self
            .symbol_flags(symbol)?
            .intersects(ast::SYMBOL_FLAGS_EXPORT_VALUE)
        {
            // If we reference an exported entity within the same module declaration, then whether
            // we prefix depends on the kind of entity. SymbolFlags.ExportHasLocal encompasses all the
            // kinds that we do NOT prefix.
            let export_symbol = self
                .symbol_export_symbol(symbol)
                .and_then(|export_symbol| self.get_merged_symbol(Some(export_symbol)))?;
            if !prefix_locals
                && self
                    .symbol_flags(export_symbol)?
                    .intersects(ast::SYMBOL_FLAGS_EXPORT_HAS_LOCAL)
                && !self
                    .symbol_flags(export_symbol)?
                    .intersects(ast::SYMBOL_FLAGS_VARIABLE)
            {
                return None;
            }
            symbol = export_symbol;
        }

        let parent_symbol = self.get_parent_of_symbol(Some(symbol))?;
        if self
            .symbol_flags(parent_symbol)?
            .intersects(ast::SYMBOL_FLAGS_VALUE_MODULE)
            && self
                .symbol_value_declaration(parent_symbol)
                .is_some_and(|declaration| self.store.kind(declaration) == ast::Kind::SourceFile)
        {
            let symbol_file = self.symbol_value_declaration(parent_symbol)?;
            let reference_file = ast::get_source_file_node_of_node(self.store, Some(node));
            // If `node` accesses an export and that export isn't in the same file, then symbol is a namespace export, so return nil.
            let symbol_is_umd_export = reference_file != Some(symbol_file);
            if symbol_is_umd_export {
                return None;
            }
            return Some(symbol_file);
        }

        let mut current = self.store.parent(node_handle);
        while let Some(ancestor) = current {
            let is_matching_container = (self.store.kind(ancestor) == ast::Kind::ModuleDeclaration
                || self.store.kind(ancestor) == ast::Kind::EnumDeclaration)
                && self.get_symbol_of_declaration(Some(ancestor)) == Some(parent_symbol);
            if is_matching_container {
                return Some(ancestor);
            }
            current = self.store.parent(ancestor);
        }
        None
    }

    pub fn get_referenced_import_declaration(
        &mut self,
        node: ast::IdentifierNode,
    ) -> Option<ast::Declaration> {
        let symbol =
            self.get_referenced_value_symbol(node, false /* startInDeclarationContainer */)?;
        // We should only get the declaration of an alias if there isn't a local value
        // declaration for the symbol
        if self.is_non_local_alias(symbol, ast::SYMBOL_FLAGS_VALUE)
            && !self.is_type_only_alias_declaration(Some(symbol))
        {
            return self
                .get_declaration_of_alias_symbol(symbol)
                .map(|node| node);
        }
        None
    }

    pub fn get_referenced_value_declaration(
        &mut self,
        node: ast::IdentifierNode,
    ) -> Option<ast::Declaration> {
        let symbol =
            self.get_referenced_value_symbol(node, false /* startInDeclarationContainer */)?;
        self.get_export_symbol_of_value_symbol_if_exported(Some(symbol))
            .and_then(|symbol| self.symbol_value_declaration(symbol))
            .map(|node| node)
    }

    pub fn get_referenced_value_declarations(
        &mut self,
        node: ast::IdentifierNode,
    ) -> Vec<ast::Declaration> {
        let mut declarations = Vec::new();
        let Some(symbol) =
            self.get_referenced_value_symbol(node, false /* startInDeclarationContainer */)
        else {
            return declarations;
        };
        let Some(symbol) = self.get_export_symbol_of_value_symbol_if_exported(Some(symbol)) else {
            return declarations;
        };

        self.with_symbol_declarations(symbol, |symbol_declarations| {
            for declaration in symbol_declarations.iter().copied() {
                match self.store.kind(declaration) {
                    ast::Kind::VariableDeclaration
                    | ast::Kind::Parameter
                    | ast::Kind::BindingElement
                    | ast::Kind::PropertyDeclaration
                    | ast::Kind::PropertyAssignment
                    | ast::Kind::ShorthandPropertyAssignment
                    | ast::Kind::EnumMember
                    | ast::Kind::ObjectLiteralExpression
                    | ast::Kind::FunctionDeclaration
                    | ast::Kind::FunctionExpression
                    | ast::Kind::ArrowFunction
                    | ast::Kind::ClassDeclaration
                    | ast::Kind::ClassExpression
                    | ast::Kind::EnumDeclaration
                    | ast::Kind::MethodDeclaration
                    | ast::Kind::GetAccessor
                    | ast::Kind::SetAccessor
                    | ast::Kind::ModuleDeclaration => {
                        declarations.push(declaration);
                    }
                    _ => {}
                }
            }
        });
        declarations
    }

    pub fn get_element_access_expression_name(&mut self, expression: ast::Node) -> String {
        if let Some(hook) = self.hooks.get_element_access_expression_name.as_ref() {
            let (name, ok) = hook(expression);
            if ok {
                return name;
            }
        }
        String::new()
    }

    pub fn get_referenced_member_value_declaration(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::Declaration> {
        // member references are `this.something` or `this[something]`, so should always simply have a resolved symbol
        let mut symbol = self.get_resolved_symbol(Some(node));
        let node_symbol = self.get_symbol_of_declaration(Some(node));
        if symbol.is_none() && node_symbol.is_some() {
            // might be a declaration instead of a ref, get the merged declaration symbol
            symbol = self.get_merged_symbol(node_symbol);
        }
        let symbol = symbol?;
        self.get_export_symbol_of_value_symbol_if_exported(Some(symbol))
            .and_then(|symbol| self.symbol_value_declaration(symbol))
            .map(|node| node)
    }
}

impl BinderReferenceResolver for ReferenceResolver<'_> {
    fn get_referenced_export_container(
        &mut self,
        node: ast::IdentifierNode,
        prefix_locals: bool,
    ) -> Option<ast::Node> {
        Self::get_referenced_export_container(self, node, prefix_locals)
    }

    fn get_referenced_import_declaration(
        &mut self,
        node: ast::IdentifierNode,
    ) -> Option<ast::Declaration> {
        Self::get_referenced_import_declaration(self, node)
    }

    fn get_referenced_value_declaration(
        &mut self,
        node: ast::IdentifierNode,
    ) -> Option<ast::Declaration> {
        Self::get_referenced_value_declaration(self, node)
    }

    fn get_referenced_value_declarations(
        &mut self,
        node: ast::IdentifierNode,
    ) -> Vec<ast::Declaration> {
        Self::get_referenced_value_declarations(self, node)
    }

    fn get_element_access_expression_name(&mut self, expression: ast::Node) -> String {
        Self::get_element_access_expression_name(self, expression)
    }

    fn get_referenced_member_value_declaration(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::Declaration> {
        Self::get_referenced_member_value_declaration(self, node)
    }
}

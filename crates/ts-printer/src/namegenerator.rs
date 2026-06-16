use ts_ast as ast;
use ts_collections::{
    self as collections, FastHashMap as HashMap, FastHashMapExt, FastHashSet as HashSet,
};

use crate::{
    AutoGenerateId, GeneratedIdentifierFlags,
    emitcontext::{EmitContextStateRef, original_in_state},
    utilities::{
        ensure_leading_hash, format_generated_name, make_identifier_from_module_name,
        remove_leading_hash,
    },
};

// Flags enum to track count of temp variables and a few dedicated names
pub type TempFlags = i32;

pub const TEMP_FLAGS_AUTO: TempFlags = 0x00000000; // No preferred name
pub const TEMP_FLAGS_COUNT_MASK: TempFlags = 0x0FFFFFFF; // Temp variable counter
pub const TEMP_FLAGS_I: TempFlags = 0x10000000; // Use/preference flag for '_i'

pub trait LocalNameBindingFacts {
    fn symbol_flags(&self, symbol: ast::SymbolHandle) -> ast::SymbolFlags;
    fn lookup_local(&self, node: ast::Node, name: &str) -> Option<ast::SymbolHandle>;
    fn next_container(&self, node: ast::Node) -> Option<ast::Node>;
}

#[derive(Clone)]
pub struct NameGenerator {
    pub(crate) context: Option<EmitContextStateRef>,
    file_level_identifiers: Option<HashSet<String>>,
    has_global_name: Option<fn(String) -> bool>,
    node_id_to_generated_name: HashMap<ast::NodeId, String>, // Map of generated names for specific nodes
    node_id_to_generated_private_name: HashMap<ast::NodeId, String>, // Map of generated private names for specific nodes
    auto_generated_id_to_generated_name: HashMap<AutoGenerateId, String>, // Map of generated names for temp and loop variables
    name_generation_scope: Option<Box<NameGenerationScope>>,
    private_name_generation_scope: Option<Box<NameGenerationScope>>,
    generated_names: collections::Set<String>, // NOTE: Used to match Strada, but should be moved to nameGenerationScope after port is complete.
}

#[derive(Clone, Default)]
pub(crate) struct NameGenerationScope {
    next: Option<Box<NameGenerationScope>>, // The next nameGenerationScope in the stack
    temp_flags: TempFlags,                  // TempFlags for the current name generation scope.
    formatted_name_temp_flags: HashMap<String, TempFlags>, // TempFlags for the current name generation scope.
    reserved_names: collections::Set<String>, // Names reserved in nested name generation scopes.
                                              // generated_names         collections.Set[string] // NOTE: generated names should be scoped after Strada port is complete.
}

impl Default for NameGenerator {
    fn default() -> Self {
        Self {
            context: None,
            file_level_identifiers: None,
            has_global_name: None,
            node_id_to_generated_name: HashMap::new(),
            node_id_to_generated_private_name: HashMap::new(),
            auto_generated_id_to_generated_name: HashMap::new(),
            name_generation_scope: None,
            private_name_generation_scope: None,
            generated_names: collections::Set::default(),
        }
    }
}

impl NameGenerator {
    pub(crate) fn set_source_file(
        &mut self,
        source_file: Option<&ast::SourceFile>,
        has_global_name: Option<fn(String) -> bool>,
    ) {
        self.file_level_identifiers =
            source_file.map(|source_file| source_file.identifiers().keys().cloned().collect());
        self.has_global_name = has_global_name;
    }

    pub fn push_scope(&mut self, reuse_temp_variable_scope: bool) {
        self.private_name_generation_scope = Some(Box::new(NameGenerationScope {
            next: self.private_name_generation_scope.take(),
            ..Default::default()
        }));
        if !reuse_temp_variable_scope {
            self.name_generation_scope = Some(Box::new(NameGenerationScope {
                next: self.name_generation_scope.take(),
                ..Default::default()
            }));
        }
    }

    pub fn pop_scope(&mut self, reuse_temp_variable_scope: bool) {
        if let Some(scope) = self.private_name_generation_scope.take() {
            self.private_name_generation_scope = scope.next;
        }
        if !reuse_temp_variable_scope {
            if let Some(scope) = self.name_generation_scope.take() {
                self.name_generation_scope = scope.next;
            }
        }
    }

    fn get_scope_mut(&mut self, private_name: bool) -> &mut Option<Box<NameGenerationScope>> {
        if private_name {
            &mut self.private_name_generation_scope
        } else {
            &mut self.name_generation_scope
        }
    }

    fn get_temp_flags(&self, private_name: bool) -> TempFlags {
        let scope = if private_name {
            &self.private_name_generation_scope
        } else {
            &self.name_generation_scope
        };
        scope
            .as_ref()
            .map(|scope| scope.temp_flags)
            .unwrap_or(TEMP_FLAGS_AUTO)
    }

    fn set_temp_flags(&mut self, private_name: bool, flags: TempFlags) {
        let scope = self.get_scope_mut(private_name);
        if scope.is_none() {
            *scope = Some(Box::new(NameGenerationScope::default()));
        }
        scope.as_mut().unwrap().temp_flags = flags;
    }

    // Gets the TempFlags to use in the current nameGenerationScope for the given key
    fn get_temp_flags_for_formatted_name(
        &self,
        private_name: bool,
        formatted_name_key: &str,
    ) -> TempFlags {
        let scope = if private_name {
            &self.private_name_generation_scope
        } else {
            &self.name_generation_scope
        };
        scope
            .as_ref()
            .and_then(|scope| {
                scope
                    .formatted_name_temp_flags
                    .get(formatted_name_key)
                    .copied()
            })
            .unwrap_or(TEMP_FLAGS_AUTO)
    }

    // Sets the TempFlags to use in the current nameGenerationScope for the given key
    fn set_temp_flags_for_formatted_name(
        &mut self,
        private_name: bool,
        formatted_name_key: &str,
        flags: TempFlags,
    ) {
        let scope = self.get_scope_mut(private_name);
        if scope.is_none() {
            *scope = Some(Box::new(NameGenerationScope::default()));
        }
        scope
            .as_mut()
            .unwrap()
            .formatted_name_temp_flags
            .insert(formatted_name_key.to_string(), flags);
    }

    fn reserve_name(&mut self, name: &str, private_name: bool, scoped: bool, temp: bool) {
        let scope = self.get_scope_mut(private_name);
        if scope.is_none() {
            *scope = Some(Box::new(NameGenerationScope::default()));
        }
        if private_name || scoped {
            scope.as_mut().unwrap().reserved_names.add(name.to_string());
        } else if !temp {
            self.generated_names.add(name.to_string()); // NOTE: Matches Strada, but is incorrect.
            // (*scope).generatedNames.Add(name) // TODO: generated names should be scoped after Strada port is complete.
        }
    }

    // Generate the text for a generated identifier or private identifier
    pub fn generate_name(&mut self, store: &ast::AstStore, name: &ast::Node) -> String {
        self.generate_name_with_resolver(store, name, |_| store)
    }

    pub fn generate_name_with_resolver<'a>(
        &mut self,
        name_store: &'a ast::AstStore,
        name: &ast::Node,
        mut store_for_node: impl FnMut(ast::Node) -> &'a ast::AstStore,
    ) -> String {
        self.generate_name_with_resolver_and_binding_facts(
            name_store,
            name,
            &mut store_for_node,
            None,
        )
    }

    pub fn generate_name_with_resolver_and_binding_facts<'a>(
        &mut self,
        name_store: &'a ast::AstStore,
        name: &ast::Node,
        mut store_for_node: impl FnMut(ast::Node) -> &'a ast::AstStore,
        binding_facts: Option<&dyn LocalNameBindingFacts>,
    ) -> String {
        if let Some(context) = self.context.clone() {
            let auto_generate = context.borrow().auto_generate.get_cloned(node_key(name));
            if let Some(auto_generate) = auto_generate {
                if auto_generate.flags.is_node() {
                    // Node names generate unique names based on their original node
                    // and are cached based on that node's id.
                    let source_node = get_node_for_generated_name_worker_with_resolver(
                        &context,
                        auto_generate.node,
                        auto_generate.id,
                        &mut store_for_node,
                    );
                    let source_store = store_for_node(source_node);
                    return self.generate_name_for_node_cached(
                        source_store,
                        &source_node,
                        &mut store_for_node,
                        ast::is_private_identifier(name_store, *name),
                        auto_generate.flags,
                        &auto_generate.prefix,
                        &auto_generate.suffix,
                        binding_facts,
                    );
                } else {
                    // Auto, Loop, and Unique names are cached based on their unique autoGenerateId.
                    if let Some(auto_generated_name) = self
                        .auto_generated_id_to_generated_name
                        .get(&auto_generate.id)
                    {
                        return auto_generated_name.clone();
                    }
                    let auto_generated_name = self.make_name(name_store, name);
                    self.auto_generated_id_to_generated_name
                        .insert(auto_generate.id, auto_generated_name.clone());
                    return auto_generated_name;
                }
            }
        }
        name_store.text(*name)
    }

    fn generate_name_for_node_cached<'a>(
        &mut self,
        store: &'a ast::AstStore,
        node: &ast::Node,
        store_for_node: &mut impl FnMut(ast::Node) -> &'a ast::AstStore,
        private_name: bool,
        flags: GeneratedIdentifierFlags,
        prefix: &str,
        suffix: &str,
        binding_facts: Option<&dyn LocalNameBindingFacts>,
    ) -> String {
        let node_id = ast::get_node_id(store, *node);
        if private_name {
            if let Some(name) = self.node_id_to_generated_private_name.get(&node_id) {
                return name.clone();
            }
            let name = self.generate_name_for_node(
                store,
                node,
                store_for_node,
                true,
                flags,
                prefix,
                suffix,
                binding_facts,
            );
            self.node_id_to_generated_private_name
                .insert(node_id, name.clone());
            name
        } else {
            if let Some(name) = self.node_id_to_generated_name.get(&node_id) {
                return name.clone();
            }
            let name = self.generate_name_for_node(
                store,
                node,
                store_for_node,
                false,
                flags,
                prefix,
                suffix,
                binding_facts,
            );
            self.node_id_to_generated_name.insert(node_id, name.clone());
            name
        }
    }

    fn generate_name_for_node<'a>(
        &mut self,
        store: &'a ast::AstStore,
        node: &ast::Node,
        store_for_node: &mut impl FnMut(ast::Node) -> &'a ast::AstStore,
        private_name: bool,
        flags: GeneratedIdentifierFlags,
        prefix: &str,
        suffix: &str,
        binding_facts: Option<&dyn LocalNameBindingFacts>,
    ) -> String {
        match store.kind(*node) {
            ast::Kind::Identifier | ast::Kind::PrivateIdentifier => {
                let base_name = self.get_text_of_node(store, node, store_for_node, binding_facts);
                self.make_unique_name(
                    &base_name,
                    false,
                    flags.is_optimistic(),
                    flags.is_reserved_in_nested_scopes(),
                    private_name,
                    prefix,
                    suffix,
                )
            }
            ast::Kind::ModuleDeclaration | ast::Kind::EnumDeclaration => {
                if private_name || !prefix.is_empty() || !suffix.is_empty() {
                    panic!(
                        "Generated name for a module or enum cannot be private and may have neither a prefix nor suffix"
                    )
                }
                self.generate_name_for_module_or_enum(store, node, binding_facts)
            }
            ast::Kind::ImportDeclaration
            | ast::Kind::JSImportDeclaration
            | ast::Kind::ExportDeclaration => {
                if private_name || !prefix.is_empty() || !suffix.is_empty() {
                    panic!(
                        "Generated name for an import or export cannot be private and may have neither a prefix nor suffix"
                    )
                }
                self.generate_name_for_import_or_export_declaration(store, node)
            }
            ast::Kind::FunctionDeclaration | ast::Kind::ClassDeclaration => {
                if private_name || !prefix.is_empty() || !suffix.is_empty() {
                    panic!(
                        "Generated name for a class or function declaration cannot be private and may have neither a prefix nor suffix"
                    )
                }
                if let Some(name) = store.name(*node) {
                    if !self.context.as_ref().is_some_and(|context| {
                        context.borrow().auto_generate.contains_key(node_key(&name))
                    }) {
                        return self.generate_name_for_node(
                            store,
                            &name,
                            store_for_node,
                            false,
                            flags,
                            "",
                            "",
                            binding_facts,
                        );
                    }
                }
                self.generate_name_for_export_default()
            }
            ast::Kind::ExportAssignment => {
                if private_name || !prefix.is_empty() || !suffix.is_empty() {
                    panic!(
                        "Generated name for an export assignment cannot be private and may have neither a prefix nor suffix"
                    )
                }
                self.generate_name_for_export_default()
            }
            ast::Kind::ClassExpression => {
                if private_name || !prefix.is_empty() || !suffix.is_empty() {
                    panic!(
                        "Generated name for a class expression cannot be private and may have neither a prefix nor suffix"
                    )
                }
                self.generate_name_for_class_expression()
            }
            ast::Kind::MethodDeclaration | ast::Kind::GetAccessor | ast::Kind::SetAccessor => self
                .generate_name_for_method_or_accessor(
                    store,
                    node,
                    store_for_node,
                    private_name,
                    prefix,
                    suffix,
                    binding_facts,
                ),
            ast::Kind::ComputedPropertyName => {
                self.make_temp_variable_name(TEMP_FLAGS_AUTO, true, private_name, prefix, suffix)
            }
            _ => self.make_temp_variable_name(TEMP_FLAGS_AUTO, false, private_name, prefix, suffix),
        }
    }

    fn get_text_of_node<'a>(
        &mut self,
        store: &'a ast::AstStore,
        node: &ast::Node,
        store_for_node: &mut impl FnMut(ast::Node) -> &'a ast::AstStore,
        binding_facts: Option<&dyn LocalNameBindingFacts>,
    ) -> String {
        if ast::is_member_name(store, *node)
            && let Some(context) = self.context.clone()
        {
            let auto_generate = context.borrow().auto_generate.get_cloned(node_key(node));
            if let Some(auto_generate) = auto_generate {
                if auto_generate.flags.is_node() {
                    let source_node = get_node_for_generated_name_worker_with_resolver(
                        &context,
                        auto_generate.node,
                        auto_generate.id,
                        store_for_node,
                    );
                    let source_store = store_for_node(source_node);
                    return self.generate_name_for_node_cached(
                        source_store,
                        &source_node,
                        store_for_node,
                        ast::is_private_identifier(store, *node),
                        auto_generate.flags,
                        &auto_generate.prefix,
                        &auto_generate.suffix,
                        binding_facts,
                    );
                } else if auto_generate.flags.is_auto()
                    || auto_generate.flags.is_loop()
                    || auto_generate.flags.is_unique()
                {
                    if let Some(auto_generated_name) = self
                        .auto_generated_id_to_generated_name
                        .get(&auto_generate.id)
                    {
                        return auto_generated_name.clone();
                    }
                    let auto_generated_name = self.make_name(store, node);
                    self.auto_generated_id_to_generated_name
                        .insert(auto_generate.id, auto_generated_name.clone());
                    return auto_generated_name;
                }
            }
        }
        store.text(*node)
    }

    fn generate_name_for_module_or_enum(
        &mut self,
        store: &ast::AstStore,
        node: &ast::Node,
        binding_facts: Option<&dyn LocalNameBindingFacts>,
    ) -> String {
        let name_node = store.name(*node).expect("module/enum name");
        let name = store.text(name_node);
        // Use module/enum name itself if it is unique, otherwise make a unique variation
        if is_unique_local_name_with_binding_facts(store, &name, node, binding_facts) {
            name
        } else {
            self.make_unique_name(&name, false, false, false, false, "", "")
        }
    }

    fn generate_name_for_import_or_export_declaration(
        &mut self,
        store: &ast::AstStore,
        node: &ast::Node,
    ) -> String {
        let expr = ast::get_external_module_name(store, *node).unwrap_or(*node);
        let mut base_name = "module".to_string();
        if ast::is_string_literal(store, expr) {
            base_name = make_identifier_from_module_name(&store.text(expr));
        }
        self.make_unique_name(&base_name, false, false, false, false, "", "")
    }

    fn generate_name_for_export_default(&mut self) -> String {
        self.make_unique_name("default", false, false, false, false, "", "")
    }

    fn generate_name_for_class_expression(&mut self) -> String {
        self.make_unique_name("class", false, false, false, false, "", "")
    }

    fn generate_name_for_method_or_accessor<'a>(
        &mut self,
        store: &'a ast::AstStore,
        node: &ast::Node,
        store_for_node: &mut impl FnMut(ast::Node) -> &'a ast::AstStore,
        private_name: bool,
        prefix: &str,
        suffix: &str,
        binding_facts: Option<&dyn LocalNameBindingFacts>,
    ) -> String {
        if let Some(name) = store.name(*node) {
            if ast::is_identifier(store, name) {
                return self.generate_name_for_node_cached(
                    store,
                    &name,
                    store_for_node,
                    private_name,
                    GeneratedIdentifierFlags::NONE,
                    prefix,
                    suffix,
                    binding_facts,
                );
            }
        }
        self.make_temp_variable_name(TEMP_FLAGS_AUTO, false, private_name, prefix, suffix)
    }

    fn make_name(&mut self, store: &ast::AstStore, name: &ast::Node) -> String {
        if let Some(context) = self.context.clone() {
            let auto_generate = context.borrow().auto_generate.get_cloned(node_key(name));
            if let Some(auto_generate) = auto_generate {
                if auto_generate.flags.is_auto() {
                    return self.make_temp_variable_name(
                        TEMP_FLAGS_AUTO,
                        auto_generate.flags.is_reserved_in_nested_scopes(),
                        ast::is_private_identifier(store, *name),
                        &auto_generate.prefix,
                        &auto_generate.suffix,
                    );
                } else if auto_generate.flags.is_loop() {
                    return self.make_temp_variable_name(
                        TEMP_FLAGS_I,
                        auto_generate.flags.is_reserved_in_nested_scopes(),
                        false,
                        &auto_generate.prefix,
                        &auto_generate.suffix,
                    );
                } else if auto_generate.flags.is_unique() {
                    return self.make_unique_name(
                        &store.text(*name),
                        auto_generate.flags.is_file_level(),
                        auto_generate.flags.is_optimistic(),
                        auto_generate.flags.is_reserved_in_nested_scopes(),
                        ast::is_private_identifier(store, *name),
                        &auto_generate.prefix,
                        &auto_generate.suffix,
                    );
                }
            }
        }
        store.text(*name)
    }

    // Return the next available name in the pattern _a ... _z, _0, _1, ...
    // TempFlags._i may be used to express a preference for that dedicated name.
    // Note that names generated by makeTempVariableName and makeUniqueName will never conflict.
    fn make_temp_variable_name(
        &mut self,
        flags: TempFlags,
        reserved_in_nested_scopes: bool,
        private_name: bool,
        prefix: &str,
        suffix: &str,
    ) -> String {
        let simple = prefix.is_empty() && suffix.is_empty();
        let (mut temp_flags, key) = if simple {
            (self.get_temp_flags(private_name), String::new())
        } else {
            // Generate a key to use to acquire a TempFlags counter based on the fixed portions of the generated name.
            let mut key = format_generated_name(private_name, prefix, "", suffix);
            if private_name {
                key = ensure_leading_hash(&key);
            }
            (
                self.get_temp_flags_for_formatted_name(private_name, &key),
                key,
            )
        };

        if flags != 0 && (temp_flags & flags) == 0 {
            let full_name = format_generated_name(private_name, prefix, "_i", suffix);
            if self.is_unique_name(&full_name, private_name) {
                temp_flags |= flags;
                self.reserve_name(&full_name, private_name, reserved_in_nested_scopes, true);
                if simple {
                    self.set_temp_flags(private_name, temp_flags);
                } else {
                    self.set_temp_flags_for_formatted_name(private_name, &key, temp_flags);
                }
                return full_name;
            }
        }

        loop {
            let count = temp_flags & TEMP_FLAGS_COUNT_MASK;
            temp_flags += 1;
            // Skip over 'i' and 'n'
            if count != 8 && count != 13 {
                let name = if count < 26 {
                    format!("_{}", (b'a' + count as u8) as char)
                } else {
                    format!("_{}", count - 26)
                };
                let full_name = format_generated_name(private_name, prefix, &name, suffix);
                if self.is_unique_name(&full_name, private_name) {
                    self.reserve_name(&full_name, private_name, reserved_in_nested_scopes, true);
                    if simple {
                        self.set_temp_flags(private_name, temp_flags);
                    } else {
                        self.set_temp_flags_for_formatted_name(private_name, &key, temp_flags);
                    }
                    return full_name;
                }
            }
        }
    }

    // Generate a name that is unique within the current file and doesn't conflict with any names
    // in global scope. The name is formed by adding an '_n' suffix to the specified base name,
    // where n is a positive integer. Note that names generated by makeTempVariableName and
    // makeUniqueName are guaranteed to never conflict.
    // If `optimistic` is set, the first instance will use 'baseName' verbatim instead of 'baseName_1'
    fn make_unique_name(
        &mut self,
        base_name: &str,
        file_level: bool,
        optimistic: bool,
        scoped: bool,
        private_name: bool,
        prefix: &str,
        suffix: &str,
    ) -> String {
        let mut base_name = remove_leading_hash(base_name);
        if optimistic {
            let full_name = format_generated_name(private_name, prefix, &base_name, suffix);
            if self.check_unique_name(&full_name, private_name, file_level) {
                self.reserve_name(&full_name, private_name, scoped, false);
                return full_name;
            }
        }

        // Find the first unique 'name_n', where n is a positive integer
        if !base_name.is_empty() && !base_name.ends_with('_') {
            base_name.push('_');
        }

        let mut i = 1;
        loop {
            let full_name =
                format_generated_name(private_name, prefix, &format!("{base_name}{i}"), suffix);
            if self.check_unique_name(&full_name, private_name, file_level) {
                self.reserve_name(&full_name, private_name, scoped, false);
                return full_name;
            }
            i += 1;
        }
    }

    pub fn make_file_level_optimistic_unique_name(&mut self, name: &str) -> String {
        self.make_unique_name(name, true, true, false, false, "", "")
    }

    fn check_unique_name(&self, name: &str, private_name: bool, file_level: bool) -> bool {
        if file_level {
            self.is_file_level_unique_name_in_current_file(name)
        } else {
            self.is_unique_name(name, private_name)
        }
    }

    fn is_file_level_unique_name_in_current_file(&self, name: &str) -> bool {
        self.has_global_name.is_none_or(|f| !f(name.to_string()))
            && self
                .file_level_identifiers
                .as_ref()
                .is_none_or(|identifiers| !identifiers.contains(name))
    }

    fn is_unique_name(&self, name: &str, private_name: bool) -> bool {
        self.is_file_level_unique_name_in_current_file(name)
            && !self.is_reserved_name(name, private_name)
    }

    fn is_reserved_name(&self, name: &str, private_name: bool) -> bool {
        let mut scope = if private_name {
            self.private_name_generation_scope.as_deref()
        } else {
            self.name_generation_scope.as_deref()
        };

        // NOTE: The following matches Strada, but is incorrect.
        if self.generated_names.has(&name.to_string()) {
            return true;
        }

        // TODO: generated names should be scoped after Strada port is complete.
        ////if *scope != nil {
        ////	if (*scope).generatedNames.Has(name) {
        ////		return true
        ////	}
        ////}

        while let Some(current) = scope {
            if current.reserved_names.has(&name.to_string()) {
                return true;
            }
            scope = current.next.as_deref();
        }
        false
    }
}

fn get_node_for_generated_name_worker_with_resolver<'a>(
    context: &EmitContextStateRef,
    mut node: ast::Node,
    auto_generate_id: AutoGenerateId,
    store_for_node: &mut impl FnMut(ast::Node) -> &'a ast::AstStore,
) -> ast::Node {
    let mut original = original_in_state(context, &node);
    while let Some(next_original) = original {
        node = next_original;
        if ast::is_member_name(store_for_node(node), node) {
            let auto_generate = context.borrow().auto_generate.get_cloned(node_key(&node));
            if auto_generate.is_none()
                || (auto_generate.as_ref().unwrap().flags.is_node()
                    && auto_generate.as_ref().unwrap().id != auto_generate_id)
            {
                break;
            }
            if auto_generate.as_ref().unwrap().flags.is_node() {
                original = Some(auto_generate.unwrap().node);
                continue;
            }
        }
        original = original_in_state(context, &node);
    }
    node
}

pub fn is_unique_local_name(store: &ast::AstStore, name: &str, container: &ast::Node) -> bool {
    is_unique_local_name_with_binding_facts(store, name, container, None)
}

pub fn is_unique_local_name_with_binding_facts(
    store: &ast::AstStore,
    name: &str,
    container: &ast::Node,
    binding_facts: Option<&dyn LocalNameBindingFacts>,
) -> bool {
    let Some(binding_facts) = binding_facts else {
        return true;
    };
    is_unique_local_name_worker(
        store,
        name,
        container,
        |symbol| binding_facts.symbol_flags(symbol),
        |node, name| binding_facts.lookup_local(node, name),
        |node| binding_facts.next_container(node),
    )
}

fn is_unique_local_name_worker(
    store: &ast::AstStore,
    name: &str,
    container: &ast::Node,
    symbol_flags: impl Fn(ast::SymbolHandle) -> ast::SymbolFlags,
    mut lookup_local: impl FnMut(ast::Node, &str) -> Option<ast::SymbolHandle>,
    mut next_container: impl FnMut(ast::Node) -> Option<ast::Node>,
) -> bool {
    let mut node = Some(*container);
    while let Some(current) = node {
        if !ast::is_node_descendant_of(store, Some(current), Some(*container))
            || !store.has_locals_container_base(current)
        {
            break;
        }
        // We conservatively include alias symbols to cover cases where they're emitted as locals.
        if let Some(local) = lookup_local(current, name)
            && symbol_flags(local)
                & (ast::SYMBOL_FLAGS_VALUE
                    | ast::SYMBOL_FLAGS_EXPORT_VALUE
                    | ast::SYMBOL_FLAGS_ALIAS)
                != ast::SYMBOL_FLAGS_NONE
        {
            return false;
        }
        node = next_container(current);
    }
    true
}

fn node_key(node: &ast::Node) -> ast::Node {
    *node
}

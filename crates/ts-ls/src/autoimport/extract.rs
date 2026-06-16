use std::cell::{RefCell, RefMut};
use std::sync::atomic::{AtomicI32, Ordering};

use ts_ast as ast;
use ts_checker as checker;
use ts_core as core;
use ts_module as module;
use ts_modulespecifiers::CheckerShape;
use ts_tspath as tspath;

use crate::autoimport::{Export, ExportId, ExportSyntax, ModuleId};
use crate::lsutil;

pub struct SymbolExtractor<'a, 'checker, 'state> {
    pub package_name: String,
    pub stats: ExtractorStats,
    pub checker: RefCell<&'checker mut checker::Checker<'a, 'state>>,
    pub to_path: Option<Box<dyn Fn(String) -> tspath::Path + Send + Sync>>,
    // realpath, if set, is used to resolve symlinks for ModuleID generation.
    // This ensures that symlinked packages use their realpath as ModuleID,
    // deduplicating exports from files that appear via multiple symlink paths.
    pub realpath: Option<Box<dyn Fn(String) -> String + Send + Sync>>,
}

pub struct ExportExtractor<'a, 'checker, 'state> {
    pub symbol_extractor: SymbolExtractor<'a, 'checker, 'state>,
    pub module_resolver: module::Resolver,
}

pub struct ExtractorStats {
    pub exports: AtomicI32,
    pub used_checker: AtomicI32,
}

impl Default for ExtractorStats {
    fn default() -> Self {
        Self {
            exports: AtomicI32::new(0),
            used_checker: AtomicI32::new(0),
        }
    }
}

impl ExportExtractor<'_, '_, '_> {
    pub fn stats(&self) -> &ExtractorStats {
        &self.symbol_extractor.stats
    }
}

pub struct CheckerUsage<'a, 'checker, 'state, 'b> {
    pub used: bool,
    pub checker: &'b RefCell<&'checker mut checker::Checker<'a, 'state>>,
}

impl<'a, 'state> CheckerUsage<'a, '_, 'state, '_> {
    pub fn peek_checker(&self) -> RefMut<'_, checker::Checker<'a, 'state>> {
        RefMut::map(self.checker.borrow_mut(), |checker| &mut **checker)
    }

    pub fn get_checker(&mut self) -> RefMut<'_, checker::Checker<'a, 'state>> {
        self.used = true;
        RefMut::map(self.checker.borrow_mut(), |checker| &mut **checker)
    }

    pub fn try_checker(&mut self) -> Option<RefMut<'_, checker::Checker<'a, 'state>>> {
        if self.used {
            return Some(RefMut::map(self.checker.borrow_mut(), |checker| {
                &mut **checker
            }));
        }
        None
    }
}

pub fn new_symbol_extractor<'a, 'checker, 'state>(
    package_name: String,
    checker: &'checker mut checker::Checker<'a, 'state>,
    to_path: Option<Box<dyn Fn(String) -> tspath::Path + Send + Sync>>,
    realpath: Option<Box<dyn Fn(String) -> String + Send + Sync>>,
) -> SymbolExtractor<'a, 'checker, 'state> {
    SymbolExtractor {
        package_name,
        checker: RefCell::new(checker),
        stats: ExtractorStats::default(),
        to_path,
        realpath,
    }
}

pub fn new_export_extractor<'a, 'checker, 'state>(
    package_name: String,
    checker: &'checker mut checker::Checker<'a, 'state>,
    module_resolver: module::Resolver,
    to_path: Option<Box<dyn Fn(String) -> tspath::Path + Send + Sync>>,
    realpath: Option<Box<dyn Fn(String) -> String + Send + Sync>>,
) -> ExportExtractor<'a, 'checker, 'state> {
    ExportExtractor {
        symbol_extractor: new_symbol_extractor(package_name, checker, to_path, realpath),
        module_resolver,
    }
}

impl SymbolExtractor<'_, '_, '_> {
    fn symbol_flags(&self, symbol: ast::SymbolIdentity) -> Option<ast::SymbolFlags> {
        self.checker.borrow_mut().symbol_flags_public(symbol)
    }

    fn collect_symbol_declarations(&self, symbol: ast::SymbolIdentity) -> Vec<ast::Node> {
        self.checker
            .borrow_mut()
            .collect_symbol_declarations_public(symbol)
    }

    fn should_ignore_symbol(&self, symbol: ast::SymbolIdentity) -> bool {
        should_ignore_symbol_flags(self.symbol_flags(symbol).unwrap_or(ast::SYMBOL_FLAGS_NONE))
    }

    fn source_symbol_exports_snapshot(
        &self,
        symbol: ast::SymbolIdentity,
    ) -> Vec<(ast::SymbolName, ast::SymbolIdentity)> {
        self.checker
            .borrow_mut()
            .symbol_exports_snapshot_public(symbol)
    }

    // getModuleID returns the ModuleID for a file, using realpath if available.
    pub fn get_module_id(
        &self,
        file: &(impl ast::SourceFileStoreLike + ast::HasFileName),
    ) -> ModuleId {
        if let (Some(realpath), Some(to_path)) = (&self.realpath, &self.to_path) {
            let realpath = realpath(file.file_name().to_string());
            return to_path(realpath);
        }
        file.path()
    }

    pub fn extract_from_symbol_identity(
        &self,
        name: &str,
        symbol: ast::SymbolIdentity,
        module_id: ModuleId,
        module_file_name: &str,
        file: &(impl ast::SourceFileStoreLike + ast::HasFileName),
        exports: &mut Vec<Export>,
    ) {
        self.extract_from_symbol(name, symbol, module_id, module_file_name, file, exports);
    }

    fn extract_from_symbol(
        &self,
        name: &str,
        symbol: ast::SymbolIdentity,
        module_id: ModuleId,
        module_file_name: &str,
        file: &(impl ast::SourceFileStoreLike + ast::HasFileName),
        exports: &mut Vec<Export>,
    ) {
        if self.should_ignore_symbol(symbol) {
            return;
        }

        if name == ast::INTERNAL_SYMBOL_NAME_EXPORT_STAR {
            let mut checker_usage = CheckerUsage {
                used: false,
                checker: &self.checker,
            };
            let parent_symbol = checker_usage
                .peek_checker()
                .symbol_parent_public(symbol)
                .expect("export-star symbol should have a parent module");
            let mut all_exports = checker_usage
                .get_checker()
                .get_exports_of_module_public(parent_symbol);
            // allExports includes named exports from the file that will be processed separately;
            // we want to add only the ones that come from the star
            for (inner_name, named_export) in self.source_symbol_exports_snapshot(parent_symbol) {
                if inner_name != ast::INTERNAL_SYMBOL_NAME_EXPORT_STAR {
                    let idx = all_exports
                        .iter()
                        .position(|export| *export == named_export);
                    let should_ignore = self.should_ignore_symbol(named_export);
                    if idx.is_some() || should_ignore {
                        all_exports
                            .remove(idx.expect("named export should be present in allExports"));
                    }
                }
            }

            exports.reserve(all_exports.len());
            for reexported_symbol in all_exports {
                let Some(reexported_symbol_name) = checker_usage
                    .peek_checker()
                    .symbol_name_public(reexported_symbol)
                else {
                    continue;
                };
                let (mut export, _) = self.create_export(
                    reexported_symbol,
                    module_id.clone(),
                    module_file_name,
                    ExportSyntax::Star,
                    file,
                    &mut checker_usage,
                );
                if let Some(export) = export.as_mut() {
                    let parent_symbol = checker_usage
                        .peek_checker()
                        .symbol_parent_public(reexported_symbol)
                        .expect("reexported symbol should have a parent module");
                    let mut checker = checker_usage.get_checker();
                    let Some(parent) = checker.get_merged_symbol_public(parent_symbol) else {
                        continue;
                    };
                    if checker.is_external_module_symbol_public(parent) {
                        let (target_module_id, _, ok) =
                            crate::autoimport::util::try_get_module_id_and_file_name_of_module_symbol(
                                &mut *checker,
                                parent,
                            );
                        if ok {
                            export.target = ExportId {
                                export_name: reexported_symbol_name,
                                module_id: target_module_id,
                            };
                        }
                    }
                    export.through = ast::INTERNAL_SYMBOL_NAME_EXPORT_STAR.to_string();
                }
                if let Some(export) = export {
                    exports.push(export);
                }
            }
            return;
        }

        let declarations = self.collect_symbol_declarations(symbol);
        let syntax = get_syntax_from_declarations(file.store(), &declarations);
        let mut checker_usage = CheckerUsage {
            used: false,
            checker: &self.checker,
        };
        let (export, target) = self.create_export(
            symbol,
            module_id.clone(),
            module_file_name,
            syntax,
            file,
            &mut checker_usage,
        );
        let Some(export) = export else {
            return;
        };

        exports.push(export);

        if let Some(target) = target {
            let target_flags = checker_usage
                .get_checker()
                .symbol_flags_public(target)
                .unwrap_or(ast::SYMBOL_FLAGS_NONE);
            if syntax == ExportSyntax::Equals && target_flags & ast::SYMBOL_FLAGS_NAMESPACE != 0 {
                let mut checker_usage = CheckerUsage {
                    used: false,
                    checker: &self.checker,
                };
                for (inner_name, named_export) in self.source_symbol_exports_snapshot(target) {
                    if inner_name == ast::INTERNAL_SYMBOL_NAME_EXPORT_STAR {
                        continue;
                    }
                    let (mut export, _) = self.create_export(
                        named_export,
                        module_id.clone(),
                        module_file_name,
                        syntax,
                        file,
                        &mut checker_usage,
                    );
                    if let Some(export) = export.as_mut() {
                        export.through = name.to_string();
                    }
                    if let Some(export) = export {
                        exports.push(export);
                    }
                }
            }
        } else if syntax == ExportSyntax::CommonJSModuleExports {
            if let Some(decl) = declarations.first() {
                let store = file.store();
                if let Some(expression) = store.right(*decl) {
                    if store.kind(expression) == ast::Kind::ObjectLiteralExpression {
                        // what is actually desirable here? I think it would be reasonable to only treat these as exports
                        // if *every* property is a shorthand property or identifier: identifier
                        // At least, it would be sketchy if there were any methods, computed properties...
                        let expression_symbol = checker_usage
                            .get_checker()
                            .source_node_symbol_public(expression);
                        if let Some(expression_symbol) = expression_symbol {
                            let mut checker_usage = CheckerUsage {
                                used: false,
                                checker: &self.checker,
                            };
                            for prop in store
                                .properties(expression)
                                .expect("object literal expression should have properties")
                            {
                                let Some(prop_name) = store.name(prop) else {
                                    continue;
                                };
                                if store.kind(prop) != ast::Kind::ShorthandPropertyAssignment
                                    && (!ast::is_property_assignment(store, prop)
                                        || store.kind(prop_name) != ast::Kind::Identifier)
                                {
                                    continue;
                                }
                                let prop_text = store.text(prop_name);
                                let member = checker_usage
                                    .peek_checker()
                                    .symbol_member_public(expression_symbol, &prop_text);
                                if let Some(member) = member {
                                    let (mut export, _) = self.create_export(
                                        member,
                                        module_id.clone(),
                                        module_file_name,
                                        syntax,
                                        file,
                                        &mut checker_usage,
                                    );
                                    if let Some(export) = export.as_mut() {
                                        export.through = name.to_string();
                                    }
                                    if let Some(export) = export {
                                        exports.push(export);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // createExport creates an Export for the given symbol, returning the Export and the target symbol if the export is an alias.
    fn create_export(
        &self,
        symbol: ast::SymbolIdentity,
        module_id: ModuleId,
        module_file_name: &str,
        syntax: ExportSyntax,
        file: &(impl ast::SourceFileStoreLike + ast::HasFileName),
        checker_usage: &mut CheckerUsage<'_, '_, '_, '_>,
    ) -> (Option<Export>, Option<ast::SymbolIdentity>) {
        let Some(symbol_name) = checker_usage.peek_checker().symbol_name_public(symbol) else {
            return (None, None);
        };
        let symbol_flags = checker_usage
            .peek_checker()
            .symbol_flags_public(symbol)
            .unwrap_or(ast::SYMBOL_FLAGS_NONE);
        if should_ignore_symbol_flags(symbol_flags) {
            return (None, None);
        }
        let combined_local_and_export_flags = checker_usage
            .peek_checker()
            .symbol_combined_local_and_export_flags_public(symbol)
            .unwrap_or(symbol_flags);
        let declarations = checker_usage
            .peek_checker()
            .collect_symbol_declarations_public(symbol);
        let mut export = Export {
            export_id: ExportId {
                export_name: symbol_name.clone(),
                module_id,
            },
            module_file_name: module_file_name.to_string(),
            syntax,
            flags: combined_local_and_export_flags,
            local_name: String::new(),
            through: String::new(),
            target: ExportId::default(),
            is_type_only: false,
            script_element_kind: lsutil::ScriptElementKind::Unknown,
            script_element_kind_modifiers: lsutil::ScriptElementKindModifier::NONE,
            path: file.path(),
            package_name: self.package_name.clone(),
        };

        if syntax == ExportSyntax::UMD {
            export.export_id.export_name = ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS.to_string();
            export.local_name = symbol_name.clone();
        }

        let mut target_symbol = None;
        if symbol_flags & ast::SYMBOL_FLAGS_ALIAS != 0 {
            target_symbol = self.try_resolve_symbol(
                symbol,
                symbol_flags,
                &symbol_name,
                &declarations,
                syntax,
                checker_usage,
            );
            if let Some(target_identity) = target_symbol {
                let Some(target_name) = checker_usage
                    .peek_checker()
                    .symbol_name_public(target_identity)
                else {
                    return (None, None);
                };
                let target_declarations = checker_usage
                    .peek_checker()
                    .collect_symbol_declarations_public(target_identity);
                let mut decl = target_declarations.first().cloned();
                let target_check_flags = checker_usage
                    .peek_checker()
                    .symbol_check_flags_public(target_identity)
                    .unwrap_or(ast::CHECK_FLAGS_NONE);
                if decl.is_none() && target_check_flags & ast::CHECK_FLAGS_MAPPED != 0 {
                    let mut checker = checker_usage.get_checker();
                    if let Some(mapped_symbol) =
                        checker.get_mapped_type_symbol_of_property_public(target_identity)
                    {
                        decl = checker
                            .collect_symbol_declarations_public(mapped_symbol)
                            .first()
                            .cloned();
                    }
                }
                if decl.is_none() {
                    // !!! consider GetImmediateAliasedSymbol to go as far as we can
                    decl = declarations.first().cloned();
                }
                let Some(decl) = decl else {
                    panic!("no declaration for aliased symbol");
                };
                let mut merged_parent = None;
                let parent = checker_usage
                    .peek_checker()
                    .symbol_parent_public(target_identity);
                if let Some(mut checker) = checker_usage.try_checker() {
                    let Some(flags) = checker.symbol_flags_public(target_identity) else {
                        return (None, None);
                    };
                    export.flags = flags;
                    export.is_type_only = checker
                        .get_type_only_alias_declaration_for_symbol_public(symbol)
                        .is_some();
                    if let Some(parent_symbol) = parent {
                        merged_parent = checker.get_merged_symbol_public(parent_symbol);
                    }
                } else {
                    export.flags = checker_usage
                        .peek_checker()
                        .symbol_flags_public(target_identity)
                        .unwrap_or(ast::SYMBOL_FLAGS_NONE);
                    export.is_type_only = declarations.iter().any(|declaration| {
                        ast::is_part_of_type_only_import_or_export_declaration(
                            file.store(),
                            declaration,
                        )
                    });
                }
                let (script_element_kind, script_element_kind_modifiers, mut target_module_id) = {
                    let mut checker = checker_usage.peek_checker();
                    let target_store = checker
                        .try_source_file_for_node_public(decl)
                        .expect("target declaration should belong to the checker program")
                        .store();
                    let script_element_kind =
                        lsutil::get_symbol_kind(target_store, &mut *checker, target_identity, decl);
                    let script_element_kind_modifiers =
                        lsutil::get_symbol_modifiers(target_store, &mut *checker, target_identity);
                    let target_module_id = ast::get_source_file_of_node(target_store, Some(decl))
                        .map(|file| target_store.as_source_file(file).path())
                        .unwrap_or_default();
                    (
                        script_element_kind,
                        script_element_kind_modifiers,
                        target_module_id,
                    )
                };
                export.script_element_kind = script_element_kind;
                export.script_element_kind_modifiers = script_element_kind_modifiers;
                if let Some(parent) = merged_parent.as_ref() {
                    let Some(mut checker) = checker_usage.try_checker() else {
                        return (None, None);
                    };
                    if checker.is_external_module_symbol_public(*parent) {
                        let (id, _, ok) =
                            crate::autoimport::util::try_get_module_id_and_file_name_of_module_symbol(
                                &mut *checker,
                                *parent,
                            );
                        if ok {
                            target_module_id = id;
                        }
                    }
                }
                export.target = ExportId {
                    export_name: target_name,
                    module_id: target_module_id,
                };
            }
        } else if let Some(first_decl) = declarations.first() {
            let mut checker = checker_usage.peek_checker();
            export.script_element_kind =
                lsutil::get_symbol_kind(file.store(), &mut *checker, symbol, *first_decl);
            export.script_element_kind_modifiers =
                lsutil::get_symbol_modifiers(file.store(), &mut *checker, symbol);
        }

        if symbol_name == ast::INTERNAL_SYMBOL_NAME_DEFAULT
            || symbol_name == ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS
        {
            export.local_name = if let Some(local_name) =
                checker_usage.try_checker().and_then(|mut checker| {
                    checker
                        .local_symbol_for_export_default_public(symbol)
                        .map(|symbol| checker.default_like_export_name_public(symbol))
                }) {
                local_name
            } else {
                get_default_like_export_name_from_symbol_identity(
                    file.store(),
                    checker_usage,
                    symbol,
                    symbol_flags,
                    &declarations,
                )
            };
            if is_unusable_name(&export.local_name) {
                export.local_name = export.target.export_name.clone();
            }
            if is_unusable_name(&export.local_name) {
                if let Some(target_symbol) = target_symbol {
                    let target_flags = checker_usage
                        .peek_checker()
                        .symbol_flags_public(target_symbol)
                        .unwrap_or(ast::SYMBOL_FLAGS_NONE);
                    let target_declarations = checker_usage
                        .peek_checker()
                        .collect_symbol_declarations_public(target_symbol);
                    export.local_name = if let Some(local_name) =
                        checker_usage.try_checker().and_then(|mut checker| {
                            checker
                                .local_symbol_for_export_default_public(target_symbol)
                                .map(|symbol| checker.default_like_export_name_public(symbol))
                        }) {
                        local_name
                    } else {
                        get_default_like_export_name_from_symbol_identity(
                            file.store(),
                            checker_usage,
                            target_symbol,
                            target_flags,
                            &target_declarations,
                        )
                    };
                    if is_unusable_name(&export.local_name) {
                        export.local_name = lsutil::module_specifier_to_valid_identifier(
                            export.target.module_id.clone(),
                            false,
                        );
                    }
                } else {
                    export.local_name = lsutil::module_specifier_to_valid_identifier(
                        export.module_id().to_string(),
                        false,
                    );
                }
            }
        }

        if is_unusable_name(export.name()) {
            return (None, None);
        }

        self.stats.exports.fetch_add(1, Ordering::Relaxed);
        if checker_usage.try_checker().is_some() {
            self.stats.used_checker.fetch_add(1, Ordering::Relaxed);
        }

        (Some(export), target_symbol)
    }

    fn try_resolve_symbol(
        &self,
        symbol: ast::SymbolIdentity,
        symbol_flags: ast::SymbolFlags,
        symbol_name: &str,
        declarations: &[ast::Node],
        syntax: ExportSyntax,
        checker_usage: &mut CheckerUsage<'_, '_, '_, '_>,
    ) -> Option<ast::SymbolIdentity> {
        if !is_non_local_alias_handle(symbol_flags, ast::SYMBOL_FLAGS_NONE) {
            return Some(symbol);
        }

        let mut loc = None;
        let mut name = String::new();
        match syntax {
            ExportSyntax::Named => {
                let checker = checker_usage.try_checker()?;
                let symbol_decl = declarations.first()?;
                let store = checker
                    .source_file_store(*symbol_decl)
                    .expect("symbol declaration should belong to the checker program");
                let decl = get_declaration_of_kind_from_declarations(
                    store,
                    declarations,
                    ast::Kind::ExportSpecifier,
                );
                if let Some(decl) = decl {
                    let export_clause = store
                        .parent(decl)
                        .expect("export specifier should have a parent");
                    let export_declaration = store
                        .parent(export_clause)
                        .expect("export clause should have a parent");
                    if store.module_specifier(export_declaration).is_none() {
                        if let Some(n) = store.name(decl).or_else(|| store.property_name(decl)) {
                            if store.kind(n) == ast::Kind::Identifier {
                                name = store.text(n);
                                loc = Some(n);
                            }
                        }
                    }
                }
            }
            // !!! check if module.exports = foo is marked as an alias
            ExportSyntax::Equals => {
                if symbol_name != ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS {
                    // Go breaks here.
                } else {
                    let checker = checker_usage.try_checker()?;
                    let symbol_decl = declarations.first()?;
                    let store = checker
                        .source_file_store(*symbol_decl)
                        .expect("symbol declaration should belong to the checker program");
                    let decl = get_declaration_of_kind_from_declarations(
                        store,
                        declarations,
                        ast::Kind::ExportAssignment,
                    )?;
                    if let Some(expression) = store.expression(decl) {
                        if store.kind(expression) == ast::Kind::Identifier {
                            name = store.text(expression);
                            loc = Some(expression);
                        }
                    }
                }
            }
            ExportSyntax::DefaultDeclaration => {
                let checker = checker_usage.try_checker()?;
                let symbol_decl = declarations.first()?;
                let store = checker
                    .source_file_store(*symbol_decl)
                    .expect("symbol declaration should belong to the checker program");
                let decl = get_declaration_of_kind_from_declarations(
                    store,
                    declarations,
                    ast::Kind::ExportAssignment,
                )?;
                if let Some(expression) = store.expression(decl) {
                    if store.kind(expression) == ast::Kind::Identifier {
                        name = store.text(expression);
                        loc = Some(expression);
                    }
                }
            }
            _ => {}
        }

        if let Some(loc) = loc {
            let mut checker = checker_usage.try_checker()?;
            let local = checker.source_node_non_alias_resolved_name_public(
                loc,
                &name,
                ast::SYMBOL_FLAGS_ALL,
            );
            if let Some(local) = local {
                return Some(local);
            }
        }

        let resolved = {
            let mut checker = checker_usage.get_checker();
            let resolved = checker.skip_alias_public(symbol)?;
            if checker.is_unknown_symbol(resolved) {
                return None;
            }
            resolved
        };
        Some(resolved)
    }
}

fn get_default_like_export_name_from_symbol_identity(
    store: &ast::AstStore,
    checker_usage: &mut CheckerUsage<'_, '_, '_, '_>,
    symbol: ast::SymbolIdentity,
    symbol_flags: ast::SymbolFlags,
    declarations: &[ast::Node],
) -> String {
    for &declaration in declarations {
        if ast::is_export_assignment(store, declaration) {
            if let Some(expression) = store.expression(declaration) {
                let inner = ast::skip_outer_expressions(store, expression, ast::OEK_ALL);
                if store.kind(inner) == ast::Kind::Identifier {
                    return store.text(inner);
                }
            }
            continue;
        }
        if ast::is_export_specifier(store, declaration)
            && symbol_flags == ast::SYMBOL_FLAGS_ALIAS
            && store.property_name(declaration).is_some()
        {
            let property_name = store.property_name(declaration).unwrap();
            if store.kind(property_name) == ast::Kind::Identifier {
                return store.text(property_name);
            }
            continue;
        }
        if let Some(name) = ast::get_name_of_declaration(store, Some(declaration)) {
            if store.kind(name) == ast::Kind::Identifier {
                return store.text(name);
            }
        }
        if let Some(parent) = checker_usage
            .peek_checker()
            .symbol_parent_public(symbol)
            .and_then(|parent| {
                let parent_flags = checker_usage
                    .peek_checker()
                    .symbol_flags_public(parent)
                    .unwrap_or(ast::SYMBOL_FLAGS_NONE);
                let parent_name = checker_usage.peek_checker().symbol_name_public(parent)?;
                Some((parent_flags, parent_name))
            })
        {
            let parent_is_external_module =
                parent.0 & ast::SYMBOL_FLAGS_MODULE != 0 && parent.1.chars().next() == Some('"');
            if !parent_is_external_module {
                return parent.1;
            }
        }
    }
    String::new()
}

fn is_non_local_alias_handle(flags: ast::SymbolFlags, excludes: ast::SymbolFlags) -> bool {
    flags & (ast::SYMBOL_FLAGS_ALIAS | excludes) == ast::SYMBOL_FLAGS_ALIAS
        || flags & ast::SYMBOL_FLAGS_ALIAS != ast::SYMBOL_FLAGS_NONE
            && flags & ast::SYMBOL_FLAGS_ASSIGNMENT != ast::SYMBOL_FLAGS_NONE
}

impl ExportExtractor<'_, '_, '_> {
    pub fn extract_from_file(&mut self, file: &ast::SourceFile) -> Vec<Export> {
        let store = file.store();
        let file_symbol = {
            let mut checker = self.symbol_extractor.checker.borrow_mut();
            checker.source_node_symbol_public(file.as_node())
        };
        if let Some(file_symbol) = file_symbol {
            return self.extract_from_module_info(file, file_symbol);
        }
        if !file.data().ambient_module_names().is_empty() {
            let statements: Vec<_> = store
                .statements(file.as_node())
                .map(|statements| statements.iter().collect())
                .unwrap_or_default();
            let module_declarations = core::filter(&statements, |decl| {
                ast::is_module_with_string_literal_name(store, *decl)
            });
            let export_count = module_declarations
                .iter()
                .filter_map(|decl| {
                    self.symbol_extractor
                        .checker
                        .borrow_mut()
                        .source_node_symbol_public(*decl)
                })
                .map(|symbol| {
                    self.symbol_extractor
                        .source_symbol_exports_snapshot(symbol)
                        .len()
                })
                .sum();
            let mut exports = Vec::with_capacity(export_count);
            for decl in module_declarations {
                self.extract_from_module_declaration(
                    decl,
                    file,
                    store
                        .name(decl)
                        .map(|name| store.text(name))
                        .unwrap_or_default(),
                    "",
                    &mut exports,
                );
            }
            return exports;
        }
        Vec::new()
    }

    fn extract_from_module_info(
        &mut self,
        file: &ast::SourceFile,
        file_symbol: ast::SymbolIdentity,
    ) -> Vec<Export> {
        let module_augmentations: Vec<ast::Node> =
            core::map_filtered(file.module_augmentations(), |name| {
                let store = file.store();
                let decl = store.parent(*name).unwrap();
                if ast::is_global_scope_augmentation(store, decl) {
                    return None;
                }
                Some(decl)
            });
        let module_id = self.symbol_extractor.get_module_id(file);
        let augmentation_export_count: usize = module_augmentations
            .iter()
            .filter_map(|decl| {
                self.symbol_extractor
                    .checker
                    .borrow_mut()
                    .source_node_symbol_public(*decl)
            })
            .map(|symbol| {
                self.symbol_extractor
                    .source_symbol_exports_snapshot(symbol)
                    .len()
            })
            .sum();
        let file_symbol_exports = self
            .symbol_extractor
            .source_symbol_exports_snapshot(file_symbol);
        let mut exports = Vec::with_capacity(file_symbol_exports.len() + augmentation_export_count);
        for (name, symbol) in file_symbol_exports {
            self.symbol_extractor.extract_from_symbol(
                &name,
                symbol,
                module_id.clone(),
                &file.file_name(),
                file,
                &mut exports,
            );
        }
        for decl in module_augmentations {
            let name = file
                .store()
                .name(decl)
                .map(|name| file.store().text(name))
                .unwrap_or_default();
            let mut module_id = name.clone();
            let mut module_file_name = String::new();
            if tspath::is_external_module_name_relative(&name) {
                let (resolved, _) = self.module_resolver.resolve_module_name(
                    &name,
                    &file.file_name(),
                    core::ModuleKind::CommonJs,
                    None,
                );
                if resolved.is_resolved() {
                    module_file_name = resolved.resolved_file_name;
                    if let Some(to_path) = self.symbol_extractor.to_path.as_ref() {
                        module_id = to_path(module_file_name.clone());
                    }
                } else {
                    // :shrug:
                    module_file_name = tspath::resolve_path(
                        &tspath::get_directory_path(&file.file_name()),
                        &[&name],
                    );
                    if let Some(to_path) = self.symbol_extractor.to_path.as_ref() {
                        module_id = to_path(module_file_name.clone());
                    }
                }
            }
            self.extract_from_module_declaration(
                decl,
                file,
                module_id,
                &module_file_name,
                &mut exports,
            );
        }
        exports
    }

    pub fn extract_from_module_declaration(
        &mut self,
        decl: ast::Node,
        file: &ast::SourceFile,
        module_id: ModuleId,
        module_file_name: &str,
        exports: &mut Vec<Export>,
    ) {
        let symbol = self
            .symbol_extractor
            .checker
            .borrow_mut()
            .source_node_symbol_public(decl);
        if let Some(symbol) = symbol {
            for (name, export_symbol) in
                self.symbol_extractor.source_symbol_exports_snapshot(symbol)
            {
                self.symbol_extractor.extract_from_symbol(
                    &name,
                    export_symbol,
                    module_id.clone(),
                    module_file_name,
                    file,
                    exports,
                );
            }
        }
    }
}

pub fn should_ignore_symbol_flags(flags: ast::SymbolFlags) -> bool {
    if flags & ast::SYMBOL_FLAGS_PROTOTYPE != 0 {
        return true;
    }
    false
}

pub fn get_syntax_from_declarations(
    store: &ast::AstStore,
    declarations: &[ast::Node],
) -> ExportSyntax {
    for decl in declarations {
        match store.kind(*decl) {
            ast::Kind::ExportSpecifier => return ExportSyntax::Named,
            ast::Kind::ExportAssignment => {
                return core::if_else(
                    store.is_export_equals(*decl).unwrap_or(false),
                    ExportSyntax::Equals,
                    ExportSyntax::DefaultDeclaration,
                );
            }
            ast::Kind::NamespaceExportDeclaration => return ExportSyntax::UMD,
            ast::Kind::BinaryExpression => match ast::get_assignment_declaration_kind(store, *decl)
            {
                ast::JSDeclarationKind::ModuleExports => {
                    return ExportSyntax::CommonJSModuleExports;
                }
                ast::JSDeclarationKind::ExportsProperty => {
                    return ExportSyntax::CommonJSExportsProperty;
                }
                _ => {}
            },
            _ => {
                if ast::get_combined_modifier_flags(store, *decl)
                    .intersects(ast::ModifierFlags::DEFAULT)
                {
                    return ExportSyntax::DefaultModifier;
                }
                return ExportSyntax::Modifier;
            }
        }
    }
    ExportSyntax::None
}

fn get_declaration_of_kind_from_declarations(
    store: &ast::AstStore,
    declarations: &[ast::Node],
    kind: ast::Kind,
) -> Option<ast::Node> {
    declarations
        .iter()
        .copied()
        .find(|declaration| store.kind(*declaration) == kind)
}

pub fn is_unusable_name(name: &str) -> bool {
    name.is_empty()
        || name == "_default"
        || name == ast::INTERNAL_SYMBOL_NAME_EXPORT_STAR
        || name == ast::INTERNAL_SYMBOL_NAME_DEFAULT
        || name == ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS
}

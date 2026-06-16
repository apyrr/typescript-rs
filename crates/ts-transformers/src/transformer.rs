use ts_ast as ast;
use ts_core as core;
use ts_printer as printer;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct FinishInstrumentationSnapshot {
    pub(crate) final_finish_count: usize,
    pub(crate) final_import_phase_count: usize,
}

#[cfg(test)]
thread_local! {
    static FINISH_INSTRUMENTATION: std::cell::Cell<FinishInstrumentationSnapshot> =
        const { std::cell::Cell::new(FinishInstrumentationSnapshot {
            final_finish_count: 0,
            final_import_phase_count: 0,
        }) };
}

#[cfg(test)]
pub(crate) fn reset_finish_instrumentation() {
    FINISH_INSTRUMENTATION.set(FinishInstrumentationSnapshot::default());
}

#[cfg(test)]
pub(crate) fn finish_instrumentation_snapshot() -> FinishInstrumentationSnapshot {
    FINISH_INSTRUMENTATION.get()
}

#[cfg(test)]
fn record_finish_instrumentation() {
    FINISH_INSTRUMENTATION.set(FINISH_INSTRUMENTATION.get().with_finish());
}

#[cfg(test)]
fn record_final_import_phase_instrumentation() {
    FINISH_INSTRUMENTATION.set(FINISH_INSTRUMENTATION.get().with_final_import_phase());
}

#[cfg(test)]
impl FinishInstrumentationSnapshot {
    fn with_finish(mut self) -> Self {
        self.final_finish_count += 1;
        self
    }

    fn with_final_import_phase(mut self) -> Self {
        self.final_import_phase_count += 1;
        self
    }
}

pub struct TransformOptions<'a> {
    pub compiler_options: &'a core::CompilerOptions,
    pub context: &'a mut printer::EmitContext,
    pub get_emit_module_format_of_file: &'a dyn Fn(&dyn ast::HasFileName) -> core::ModuleKind,
    pub module_transform_facts: crate::moduletransforms::ModuleTransformFacts,
    pub import_elision_facts:
        Option<crate::tstransforms::importelision::ImportElisionResolverFacts>,
    pub metadata_facts: Option<crate::tstransforms::metadata::MetadataResolverFacts>,
    pub runtime_syntax_facts:
        Option<crate::tstransforms::runtimesyntax::RuntimeSyntaxResolverFacts>,
    pub legacy_decorators_facts:
        Option<crate::tstransforms::legacydecorators::LegacyDecoratorsResolverFacts>,
    pub const_enum_inlining_facts: Option<crate::inliners::constenum::ConstEnumInliningFacts>,
    pub jsx_facts: Option<crate::jsxtransforms::JsxResolverFacts>,
}

pub enum SourceFileTransformer {
    UseStrict {
        compiler_options: core::CompilerOptions,
        file_module_format: core::ModuleKind,
    },
    EsModule {
        compiler_options: core::CompilerOptions,
        file_module_format: core::ModuleKind,
    },
    CommonJsModule {
        compiler_options: core::CompilerOptions,
        facts: crate::moduletransforms::ModuleTransformFacts,
    },
    ImpliedModule {
        compiler_options: core::CompilerOptions,
        file_module_format: core::ModuleKind,
        facts: crate::moduletransforms::ModuleTransformFacts,
    },
    Jsx {
        compiler_options: core::CompilerOptions,
        facts: crate::jsxtransforms::JsxResolverFacts,
    },
    RuntimeSyntax {
        compiler_options: core::CompilerOptions,
        facts: crate::tstransforms::runtimesyntax::RuntimeSyntaxResolverFacts,
    },
    Metadata {
        compiler_options: core::CompilerOptions,
        facts: crate::tstransforms::metadata::MetadataResolverFacts,
    },
    ImportElision {
        facts: crate::tstransforms::importelision::ImportElisionResolverFacts,
    },
    TypeEraser {
        compiler_options: core::CompilerOptions,
    },
    LegacyDecorators {
        compiler_options: core::CompilerOptions,
        facts: crate::tstransforms::legacydecorators::LegacyDecoratorsResolverFacts,
    },
    ConstEnumInlining {
        compiler_options: core::CompilerOptions,
        facts: crate::inliners::constenum::ConstEnumInliningFacts,
    },
    UsingDeclaration,
    EsDecorator {
        compiler_options: core::CompilerOptions,
    },
    ClassFields {
        config: crate::estransforms::classfields::ClassFieldTransformConfig,
    },
    LogicalAssignment,
    ObjectRestSpread {
        compiler_options: core::CompilerOptions,
    },
    NullishCoalescing,
    OptionalChain,
    OptionalCatch,
    ForAwait,
    TaggedTemplateLiftRestriction,
    Async,
    Exponentiation,
}

pub trait SourceFileTransform {
    fn transform_source_file(
        &mut self,
        file: TransformSourceFile<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> SourceFileTransformResult;
}

#[derive(Clone, Copy)]
pub struct TransformSourceFile<'a> {
    original: &'a ast::SourceFile,
    root: ast::Node,
}

impl<'a> TransformSourceFile<'a> {
    pub fn new(original: &'a ast::SourceFile) -> Self {
        Self {
            original,
            root: original.root(),
        }
    }

    pub fn with_root(original: &'a ast::SourceFile, root: ast::Node) -> Self {
        Self { original, root }
    }

    pub fn original_source_file(self) -> &'a ast::SourceFile {
        self.original
    }

    pub fn root(self) -> ast::Node {
        self.root
    }

    pub fn store_for_root(self, emit_context: &printer::EmitContext) -> &ast::AstStore {
        emit_context.store_for_node(self.root)
    }

    pub fn source_file_view(
        self,
        emit_context: &printer::EmitContext,
    ) -> Option<ast::SourceFileView<'_>> {
        emit_context.source_file_for_node(self.root)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceFileTransformResult {
    Unchanged,
    Root(ast::Node),
}

impl SourceFileTransformResult {
    fn from_optional_output(input_root: ast::Node, output: Option<ast::Node>) -> Self {
        match output {
            Some(root) if root != input_root => Self::Root(root),
            _ => Self::Unchanged,
        }
    }

    fn from_root(input_root: ast::Node, root: ast::Node) -> Self {
        if root == input_root {
            Self::Unchanged
        } else {
            Self::Root(root)
        }
    }
}

pub struct ChainedSourceFileTransformer {
    components: Vec<Box<dyn SourceFileTransform>>,
}

impl ChainedSourceFileTransformer {
    pub fn new(components: Vec<Box<dyn SourceFileTransform>>) -> Self {
        Self { components }
    }
}

pub struct Transformer {
    emit_context: Option<printer::EmitContext>,
    source_file_transformer: Option<Box<dyn SourceFileTransform>>,
}

impl Transformer {
    pub fn new() -> Self {
        Self {
            emit_context: None,
            source_file_transformer: None,
        }
    }

    pub fn new_source_file_transformer<T>(
        &mut self,
        transform: T,
        emit_context: Option<printer::EmitContext>,
    ) -> &mut Self
    where
        T: SourceFileTransform + 'static,
    {
        if self.source_file_transformer.is_some() {
            panic!("Transformer already initialized");
        }
        self.emit_context = emit_context;
        if let Some(emit_context) = self.emit_context.as_mut() {
            emit_context.activate();
        }
        self.source_file_transformer = Some(Box::new(transform));
        self
    }

    pub(crate) fn into_source_file_transformer(self) -> Box<dyn SourceFileTransform> {
        self.source_file_transformer
            .expect("Transformer not initialized")
    }

    pub fn emit_context(&self) -> Option<&printer::EmitContext> {
        self.emit_context.as_ref()
    }

    pub fn emit_context_mut(&mut self) -> Option<&mut printer::EmitContext> {
        self.emit_context.as_mut()
    }

    pub fn take_emit_context(&mut self) -> Option<printer::EmitContext> {
        self.emit_context.take()
    }

    pub fn factory_mut(&mut self) -> Option<&mut printer::NodeFactory> {
        self.emit_context
            .as_mut()
            .map(|context| &mut context.factory)
    }

    pub fn transform_source_file(&mut self, file: &ast::SourceFile) -> ast::SourceFile {
        let mut emit_context = self
            .emit_context
            .take()
            .unwrap_or_else(printer::new_emit_context);
        let result = self.transform_source_file_with_emit_context(file, &mut emit_context);
        self.emit_context = Some(emit_context);
        result
    }

    pub fn transform_source_file_with_emit_context(
        &mut self,
        file: &ast::SourceFile,
        emit_context: &mut printer::EmitContext,
    ) -> ast::SourceFile {
        emit_context.activate();
        emit_context.set_source_file(Some(file));
        let result = self
            .source_file_transformer
            .as_mut()
            .expect("Transformer not initialized")
            .transform_source_file(TransformSourceFile::new(file), emit_context);
        finish_source_file_transform_result(file, emit_context, result)
    }
}

pub(crate) fn finish_declaration_source_file_output(
    file: &ast::SourceFile,
    emit_context: &mut printer::EmitContext,
    root: ast::Node,
) -> ast::SourceFile {
    finish_source_file_from_emit_context(file, emit_context, root)
}

fn finish_source_file_transform_result(
    file: &ast::SourceFile,
    emit_context: &mut printer::EmitContext,
    output: SourceFileTransformResult,
) -> ast::SourceFile {
    emit_context.assert_environment_balanced();
    match output {
        SourceFileTransformResult::Root(root) => {
            finish_source_file_from_emit_context(file, emit_context, root)
        }
        SourceFileTransformResult::Unchanged => file.share_readonly(),
    }
}

fn finish_source_file_from_emit_context(
    file: &ast::SourceFile,
    emit_context: &mut printer::EmitContext,
    root: ast::Node,
) -> ast::SourceFile {
    emit_context.assert_environment_balanced();
    if root == file.root() {
        return file.share_readonly();
    }
    let root = if root.store_id() == emit_context.factory.node_factory.store().store_id() {
        root
    } else {
        assert_eq!(
            root.store_id(),
            file.store().store_id(),
            "transform output root must come from the input file or output factory store"
        );
        emit_context
            .factory
            .node_factory
            .deep_clone_node_from_store_preserve_location(file.store(), root)
    };
    #[cfg(test)]
    record_finish_instrumentation();
    #[cfg(test)]
    record_final_import_phase_instrumentation();
    emit_context.import_foreign_references_from_store(file.store());
    emit_context.import_foreign_references_from_known_source_files();
    emit_context.move_emit_helpers(&file.root(), &root, |_| true);
    let output_factory = std::mem::take(&mut emit_context.factory.node_factory);
    let output = output_factory.finish_transformed_source_file(root);
    emit_context.activate();
    output
}

impl SourceFileTransform for SourceFileTransformer {
    fn transform_source_file(
        &mut self,
        input: TransformSourceFile<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> SourceFileTransformResult {
        let input_root = input.root();
        let original = input.original_source_file();
        match self {
            Self::UseStrict {
                compiler_options,
                file_module_format,
            } => {
                let output = crate::estransforms::usestrict::visit_source_file_output(
                    original,
                    input_root,
                    emit_context,
                    compiler_options,
                    *file_module_format,
                );
                SourceFileTransformResult::from_optional_output(input_root, output)
            }
            Self::EsModule {
                compiler_options,
                file_module_format,
            } => {
                let output = crate::moduletransforms::visit_es_module_source_file_root_output(
                    original,
                    input_root,
                    emit_context,
                    compiler_options,
                    *file_module_format,
                );
                SourceFileTransformResult::from_optional_output(input_root, output)
            }
            Self::CommonJsModule {
                compiler_options,
                facts,
            } => {
                let output =
                    crate::moduletransforms::visit_common_js_module_source_file_root_output(
                        original,
                        input_root,
                        emit_context,
                        compiler_options,
                        core::ModuleKind::CommonJS,
                        facts.clone(),
                    );
                SourceFileTransformResult::from_optional_output(input_root, output)
            }
            Self::ImpliedModule {
                compiler_options,
                file_module_format,
                facts,
            } => {
                let output = crate::moduletransforms::visit_implied_module_source_file_root_output(
                    original,
                    input_root,
                    emit_context,
                    compiler_options,
                    *file_module_format,
                    facts.clone(),
                );
                SourceFileTransformResult::from_optional_output(input_root, output)
            }
            Self::Jsx {
                compiler_options,
                facts,
            } => {
                let output = crate::jsxtransforms::visit_jsx_source_file_root(
                    original,
                    input_root,
                    emit_context,
                    compiler_options,
                    facts.clone(),
                );
                SourceFileTransformResult::from_optional_output(input_root, output)
            }
            Self::RuntimeSyntax {
                compiler_options,
                facts,
            } => {
                let root = crate::tstransforms::runtimesyntax::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                    compiler_options,
                    facts,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
            Self::Metadata {
                compiler_options,
                facts,
            } => {
                let root = crate::tstransforms::metadata::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                    compiler_options,
                    facts,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
            Self::ImportElision { facts } => {
                let output = crate::tstransforms::importelision::visit_source_file_output(
                    original,
                    input_root,
                    emit_context,
                    facts,
                );
                SourceFileTransformResult::from_optional_output(input_root, output)
            }
            Self::TypeEraser { compiler_options } => {
                let root = crate::tstransforms::typeeraser::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                    compiler_options,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
            Self::LegacyDecorators {
                compiler_options,
                facts,
            } => {
                let root = crate::tstransforms::legacydecorators::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                    compiler_options,
                    facts,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
            Self::ConstEnumInlining {
                compiler_options,
                facts,
            } => {
                let root = crate::inliners::constenum::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                    compiler_options,
                    facts,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
            Self::UsingDeclaration => {
                let root = crate::estransforms::using::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
            Self::EsDecorator { compiler_options } => {
                let root = crate::estransforms::esdecorator::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                    compiler_options,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
            Self::ClassFields { config } => {
                let root = crate::estransforms::classfields::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                    *config,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
            Self::LogicalAssignment => {
                let root = crate::estransforms::logicalassignment::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
            Self::ObjectRestSpread { compiler_options } => {
                let root = crate::estransforms::objectrestspread::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                    compiler_options,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
            Self::NullishCoalescing => {
                let root = crate::estransforms::nullishcoalescing::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
            Self::OptionalChain => {
                let root = crate::estransforms::optionalchain::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
            Self::OptionalCatch => {
                let root = crate::estransforms::optionalcatch::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
            Self::ForAwait => {
                let root = crate::estransforms::forawait::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
            Self::TaggedTemplateLiftRestriction => {
                let root = crate::estransforms::taggedtemplate::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
            Self::Async => {
                let root = crate::estransforms::r#async::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
            Self::Exponentiation => {
                let root = crate::estransforms::exponentiation::visit_source_file_root(
                    original,
                    input_root,
                    emit_context,
                );
                SourceFileTransformResult::from_root(input_root, root)
            }
        }
    }
}

impl SourceFileTransform for ChainedSourceFileTransformer {
    fn transform_source_file(
        &mut self,
        file: TransformSourceFile<'_>,
        emit_context: &mut printer::EmitContext,
    ) -> SourceFileTransformResult {
        let source_file = file.original;
        let input_root = file.root();
        let mut current_root = input_root;
        for component in &mut self.components {
            let next = component.transform_source_file(
                TransformSourceFile::with_root(source_file, current_root),
                emit_context,
            );
            if let SourceFileTransformResult::Root(next_root) = next {
                current_root = next_root;
            }
        }
        SourceFileTransformResult::from_root(input_root, current_root)
    }
}

impl Default for Transformer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;
    use ts_parser as parser;

    fn source_file() -> ast::SourceFile {
        let mut factory = ast::NodeFactory::default();
        let statements = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::new(),
        );
        let file = factory.new_source_file(
            ast::SourceFileParseOptions {
                file_name: "/transformer.ts".to_string(),
                path: "/transformer.ts".to_string(),
                ..Default::default()
            },
            String::new(),
            statements,
            None,
        );
        factory.finish_parsed_source_file(file, ast::ParsedSourceFileMetadata::default())
    }

    fn parse_typescript(text: &str) -> ast::SourceFile {
        parser::parse_source_file(
            ast::SourceFileParseOptions {
                file_name: "/transformer.ts".to_string(),
                path: "/transformer.ts".to_string(),
                ..Default::default()
            },
            text.to_string(),
            core::ScriptKind::TS,
        )
    }

    fn emit_source_file(file: &ast::SourceFile, emit_context: printer::EmitContext) -> String {
        let mut printer = printer::new_printer(
            printer::PrinterOptions::default(),
            printer::PrintHandlers::default(),
            Some(emit_context),
        );
        let text = printer.emit(&file.as_node(), Some(file));
        text.strip_suffix('\n').unwrap_or(&text).to_string()
    }

    fn assert_finishes_once(counts: FinishInstrumentationSnapshot) {
        assert_eq!(
            counts,
            FinishInstrumentationSnapshot {
                final_finish_count: 1,
                final_import_phase_count: 1,
            }
        );
    }

    struct CloneRootTransform {
        output_root: Rc<Cell<Option<ast::Node>>>,
    }

    impl SourceFileTransform for CloneRootTransform {
        fn transform_source_file(
            &mut self,
            input: TransformSourceFile<'_>,
            emit_context: &mut printer::EmitContext,
        ) -> SourceFileTransformResult {
            let file = input.original_source_file();
            let root = emit_context
                .factory
                .node_factory
                .deep_clone_node_from_store_preserve_location(file.store(), input.root());
            self.output_root.set(Some(root));
            SourceFileTransformResult::Root(root)
        }
    }

    struct AssertCurrentRootTransform {
        expected_root: Rc<Cell<Option<ast::Node>>>,
    }

    impl SourceFileTransform for AssertCurrentRootTransform {
        fn transform_source_file(
            &mut self,
            input: TransformSourceFile<'_>,
            _emit_context: &mut printer::EmitContext,
        ) -> SourceFileTransformResult {
            assert_eq!(Some(input.root()), self.expected_root.get());
            SourceFileTransformResult::Unchanged
        }
    }

    struct LeakingLexicalEnvironmentTransform;

    impl SourceFileTransform for LeakingLexicalEnvironmentTransform {
        fn transform_source_file(
            &mut self,
            _input: TransformSourceFile<'_>,
            emit_context: &mut printer::EmitContext,
        ) -> SourceFileTransformResult {
            emit_context.start_lexical_environment();
            SourceFileTransformResult::Unchanged
        }
    }

    struct RecordFactoryStoreTransform {
        factory_store: Rc<Cell<Option<ast::StoreId>>>,
        output_root: Rc<Cell<Option<ast::Node>>>,
    }

    impl SourceFileTransform for RecordFactoryStoreTransform {
        fn transform_source_file(
            &mut self,
            input: TransformSourceFile<'_>,
            emit_context: &mut printer::EmitContext,
        ) -> SourceFileTransformResult {
            let factory_store = emit_context.factory.node_factory.store().store_id();
            self.factory_store.set(Some(factory_store));
            let root = emit_context
                .factory
                .node_factory
                .deep_clone_node_from_store_preserve_location(
                    input.original_source_file().store(),
                    input.root(),
                );
            self.output_root.set(Some(root));
            SourceFileTransformResult::Root(root)
        }
    }

    struct AssertSameFactoryStoreTransform {
        factory_store: Rc<Cell<Option<ast::StoreId>>>,
        expected_root: Rc<Cell<Option<ast::Node>>>,
    }

    impl SourceFileTransform for AssertSameFactoryStoreTransform {
        fn transform_source_file(
            &mut self,
            input: TransformSourceFile<'_>,
            emit_context: &mut printer::EmitContext,
        ) -> SourceFileTransformResult {
            let factory_store = self.factory_store.get().expect("recorded factory store");
            assert_eq!(
                emit_context.factory.node_factory.store().store_id(),
                factory_store
            );
            assert_eq!(input.root().store_id(), factory_store);
            assert_eq!(Some(input.root()), self.expected_root.get());
            SourceFileTransformResult::Unchanged
        }
    }

    fn transform_at_public_boundary<T: SourceFileTransform>(
        transformer: &mut T,
        file: &ast::SourceFile,
        emit_context: &mut printer::EmitContext,
    ) -> ast::SourceFile {
        emit_context.set_source_file(Some(file));
        let result =
            transformer.transform_source_file(TransformSourceFile::new(file), emit_context);
        finish_source_file_transform_result(file, emit_context, result)
    }

    fn run_two_component_script_chain() -> FinishInstrumentationSnapshot {
        reset_finish_instrumentation();
        let output_root = Rc::new(Cell::new(None));
        let mut transformer = ChainedSourceFileTransformer::new(vec![
            Box::new(CloneRootTransform {
                output_root: Rc::clone(&output_root),
            }),
            Box::new(AssertCurrentRootTransform {
                expected_root: output_root,
            }),
        ]);
        let mut emit_context = printer::new_emit_context();
        emit_context.activate();
        let file = source_file();
        let _ = transform_at_public_boundary(&mut transformer, &file, &mut emit_context);
        finish_instrumentation_snapshot()
    }

    #[test]
    fn chained_script_transformer_root_to_root_lifecycle_finishes_once() {
        let counts = run_two_component_script_chain();

        assert_eq!(
            counts,
            FinishInstrumentationSnapshot {
                final_finish_count: 1,
                final_import_phase_count: 1,
            }
        );
    }

    #[test]
    fn chained_script_transformer_keeps_one_factory_store_between_components() {
        reset_finish_instrumentation();
        let factory_store = Rc::new(Cell::new(None));
        let output_root = Rc::new(Cell::new(None));
        let mut transformer = ChainedSourceFileTransformer::new(vec![
            Box::new(RecordFactoryStoreTransform {
                factory_store: Rc::clone(&factory_store),
                output_root: Rc::clone(&output_root),
            }),
            Box::new(AssertSameFactoryStoreTransform {
                factory_store: Rc::clone(&factory_store),
                expected_root: output_root,
            }),
        ]);
        let mut emit_context = printer::new_emit_context();
        emit_context.activate();
        let file = source_file();

        let _ = transform_at_public_boundary(&mut transformer, &file, &mut emit_context);
        let counts = finish_instrumentation_snapshot();
        let next_factory_store = emit_context.factory.node_factory.store().store_id();

        assert_ne!(Some(next_factory_store), factory_store.get());
        assert_finishes_once(counts);
    }

    #[test]
    fn chained_real_script_transformers_consume_active_source_file_roots() {
        reset_finish_instrumentation();
        let source_file =
            parse_typescript("import type { Foo } from './foo';\nlet value: number = 1;");
        assert!(
            source_file.diagnostics().is_empty(),
            "unexpected parse diagnostics"
        );

        let mut transformer = ChainedSourceFileTransformer::new(vec![
            Box::new(SourceFileTransformer::TypeEraser {
                compiler_options: core::CompilerOptions::default(),
            }),
            Box::new(SourceFileTransformer::ImportElision {
                facts: crate::tstransforms::importelision::ImportElisionResolverFacts::default(),
            }),
            Box::new(SourceFileTransformer::UseStrict {
                compiler_options: core::CompilerOptions::default(),
                file_module_format: core::ModuleKind::CommonJS,
            }),
        ]);
        let mut emit_context = printer::new_emit_context();
        emit_context.activate();

        let output =
            transform_at_public_boundary(&mut transformer, &source_file, &mut emit_context);
        let counts = finish_instrumentation_snapshot();
        let emitted = emit_source_file(&output, emit_context);

        assert_eq!(emitted, "\"use strict\";\nlet value = 1;");
        assert_finishes_once(counts);
    }

    #[test]
    #[should_panic(
        expected = "emit context environments must be balanced before finishing a source file"
    )]
    fn script_finish_rejects_unbalanced_environments() {
        let file = source_file();
        let mut transformer = LeakingLexicalEnvironmentTransform;
        let mut emit_context = printer::new_emit_context();
        emit_context.activate();

        let _ = transform_at_public_boundary(&mut transformer, &file, &mut emit_context);
    }

    #[test]
    fn use_strict_then_common_js_module_consumes_active_root() {
        reset_finish_instrumentation();
        let source_file = parse_typescript("export const value = 1;");
        assert!(
            source_file.diagnostics().is_empty(),
            "unexpected parse diagnostics"
        );
        let compiler_options = core::CompilerOptions {
            module: core::ModuleKind::CommonJS,
            ..Default::default()
        };

        let mut transformer = ChainedSourceFileTransformer::new(vec![
            Box::new(SourceFileTransformer::UseStrict {
                compiler_options: compiler_options.clone(),
                file_module_format: core::ModuleKind::CommonJS,
            }),
            Box::new(SourceFileTransformer::CommonJsModule {
                compiler_options,
                facts: crate::moduletransforms::ModuleTransformFacts::default(),
            }),
        ]);
        let mut emit_context = printer::new_emit_context();
        emit_context.activate();

        let output =
            transform_at_public_boundary(&mut transformer, &source_file, &mut emit_context);
        let counts = finish_instrumentation_snapshot();
        let emitted = emit_source_file(&output, emit_context);

        assert!(emitted.contains("\"use strict\";"), "{emitted}");
        assert!(emitted.contains("exports.value"), "{emitted}");
        assert_finishes_once(counts);
    }

    #[test]
    fn use_strict_then_es_module_consumes_active_root() {
        reset_finish_instrumentation();
        let source_file = parse_typescript("import foo = require(\"foo\");\nfoo;");
        assert!(
            source_file.diagnostics().is_empty(),
            "unexpected parse diagnostics"
        );
        let use_strict_options = core::CompilerOptions {
            module: core::ModuleKind::CommonJS,
            ..Default::default()
        };
        let es_module_options = core::CompilerOptions {
            module: core::ModuleKind::Node16,
            ..Default::default()
        };

        let mut transformer = ChainedSourceFileTransformer::new(vec![
            Box::new(SourceFileTransformer::UseStrict {
                compiler_options: use_strict_options,
                file_module_format: core::ModuleKind::CommonJS,
            }),
            Box::new(SourceFileTransformer::EsModule {
                compiler_options: es_module_options,
                file_module_format: core::ModuleKind::ES2015,
            }),
        ]);
        let mut emit_context = printer::new_emit_context();
        emit_context.activate();

        let output =
            transform_at_public_boundary(&mut transformer, &source_file, &mut emit_context);
        let counts = finish_instrumentation_snapshot();
        let emitted = emit_source_file(&output, emit_context);

        assert!(emitted.contains("\"use strict\";"), "{emitted}");
        assert!(emitted.contains("createRequire"), "{emitted}");
        assert!(emitted.contains("__require(\"foo\")"), "{emitted}");
        assert_finishes_once(counts);
    }

    #[test]
    fn use_strict_then_implied_module_consumes_active_root() {
        reset_finish_instrumentation();
        let source_file = parse_typescript("export const value = 1;");
        assert!(
            source_file.diagnostics().is_empty(),
            "unexpected parse diagnostics"
        );
        let compiler_options = core::CompilerOptions {
            module: core::ModuleKind::CommonJS,
            ..Default::default()
        };

        let mut transformer = ChainedSourceFileTransformer::new(vec![
            Box::new(SourceFileTransformer::UseStrict {
                compiler_options: compiler_options.clone(),
                file_module_format: core::ModuleKind::CommonJS,
            }),
            Box::new(SourceFileTransformer::ImpliedModule {
                compiler_options,
                file_module_format: core::ModuleKind::CommonJS,
                facts: crate::moduletransforms::ModuleTransformFacts::default(),
            }),
        ]);
        let mut emit_context = printer::new_emit_context();
        emit_context.activate();

        let output =
            transform_at_public_boundary(&mut transformer, &source_file, &mut emit_context);
        let counts = finish_instrumentation_snapshot();
        let emitted = emit_source_file(&output, emit_context);

        assert!(emitted.contains("\"use strict\";"), "{emitted}");
        assert!(emitted.contains("exports.value"), "{emitted}");
        assert_finishes_once(counts);
    }

    #[test]
    fn type_eraser_before_common_js_module_strips_variable_types() {
        reset_finish_instrumentation();
        let source_file = parse_typescript("import { f } from \"mod\";\nlet value: string = f;");
        assert!(
            source_file.diagnostics().is_empty(),
            "unexpected parse diagnostics"
        );
        let compiler_options = core::CompilerOptions {
            module: core::ModuleKind::CommonJS,
            ..Default::default()
        };

        let mut transformer = ChainedSourceFileTransformer::new(vec![
            Box::new(SourceFileTransformer::TypeEraser {
                compiler_options: compiler_options.clone(),
            }),
            Box::new(SourceFileTransformer::UseStrict {
                compiler_options: compiler_options.clone(),
                file_module_format: core::ModuleKind::CommonJS,
            }),
            Box::new(SourceFileTransformer::ImpliedModule {
                compiler_options,
                file_module_format: core::ModuleKind::CommonJS,
                facts: crate::moduletransforms::ModuleTransformFacts::default(),
            }),
        ]);
        let mut emit_context = printer::new_emit_context();
        emit_context.activate();

        let output =
            transform_at_public_boundary(&mut transformer, &source_file, &mut emit_context);
        let counts = finish_instrumentation_snapshot();
        let emitted = emit_source_file(&output, emit_context);

        assert!(emitted.contains("let value ="), "{emitted}");
        assert!(!emitted.contains(": string"), "{emitted}");
        assert_finishes_once(counts);
    }

    #[test]
    fn type_eraser_before_common_js_module_strips_exported_class_member_assertions() {
        reset_finish_instrumentation();
        let source_file = parse_typescript(
            "import { TypeB } from \"./type-b\";\nexport class Broken { method() { return {} as TypeB; } }",
        );
        assert!(
            source_file.diagnostics().is_empty(),
            "unexpected parse diagnostics"
        );
        let compiler_options = core::CompilerOptions {
            module: core::ModuleKind::CommonJS,
            ..Default::default()
        };

        let mut transformer = ChainedSourceFileTransformer::new(vec![
            Box::new(SourceFileTransformer::TypeEraser {
                compiler_options: compiler_options.clone(),
            }),
            Box::new(SourceFileTransformer::ImportElision {
                facts: crate::tstransforms::importelision::ImportElisionResolverFacts::default(),
            }),
            Box::new(SourceFileTransformer::UseStrict {
                compiler_options: compiler_options.clone(),
                file_module_format: core::ModuleKind::CommonJS,
            }),
            Box::new(SourceFileTransformer::CommonJsModule {
                compiler_options,
                facts: crate::moduletransforms::ModuleTransformFacts::default(),
            }),
        ]);
        let mut emit_context = printer::new_emit_context();
        emit_context.activate();

        let output =
            transform_at_public_boundary(&mut transformer, &source_file, &mut emit_context);
        let counts = finish_instrumentation_snapshot();
        let emitted = emit_source_file(&output, emit_context);

        assert!(emitted.contains("return {};"), "{emitted}");
        assert!(!emitted.contains(" as TypeB"), "{emitted}");
        assert_finishes_once(counts);
    }

    #[test]
    fn type_script_transform_chain_drops_uninitialized_public_field_for_es2015() {
        reset_finish_instrumentation();
        let source_file = parse_typescript(
            "interface T {}\nclass C {\n    x: T;\n    get A() { return this.x; }\n}",
        );
        assert!(
            source_file.diagnostics().is_empty(),
            "unexpected parse diagnostics"
        );
        let compiler_options = core::CompilerOptions {
            target: core::ScriptTarget::ES2015,
            ..Default::default()
        };
        let config = crate::estransforms::classfields::class_field_transform_config(
            compiler_options.get_emit_script_target(),
            compiler_options.get_use_define_for_class_fields(),
            compiler_options.experimental_decorators.is_true(),
        )
        .expect("ES2015 class field transform should be enabled");

        let mut transformer = ChainedSourceFileTransformer::new(vec![
            Box::new(SourceFileTransformer::TypeEraser {
                compiler_options: compiler_options.clone(),
            }),
            Box::new(SourceFileTransformer::RuntimeSyntax {
                compiler_options: compiler_options.clone(),
                facts: crate::tstransforms::runtimesyntax::RuntimeSyntaxResolverFacts::default(),
            }),
            Box::new(SourceFileTransformer::ClassFields { config }),
        ]);
        let mut emit_context = printer::new_emit_context();
        emit_context.activate();

        let output =
            transform_at_public_boundary(&mut transformer, &source_file, &mut emit_context);
        let counts = finish_instrumentation_snapshot();
        let emitted = emit_source_file(&output, emit_context);

        assert!(!emitted.contains("x: T"), "{emitted}");
        assert!(!emitted.contains("\n    x;"), "{emitted}");
        assert!(emitted.contains("get A()"), "{emitted}");
        assert_finishes_once(counts);
    }

    #[test]
    fn type_script_transform_chain_lowers_exported_initialized_public_field_for_es2015_commonjs() {
        reset_finish_instrumentation();
        let source_file = parse_typescript(
            "export const FOO = 'FOO';\nexport class C {\n    readonly type = FOO;\n}",
        );
        assert!(
            source_file.diagnostics().is_empty(),
            "unexpected parse diagnostics"
        );
        let compiler_options = core::CompilerOptions {
            target: core::ScriptTarget::ES2015,
            module: core::ModuleKind::CommonJS,
            ..Default::default()
        };
        let config = crate::estransforms::classfields::class_field_transform_config(
            compiler_options.get_emit_script_target(),
            compiler_options.get_use_define_for_class_fields(),
            compiler_options.experimental_decorators.is_true(),
        )
        .expect("ES2015 class field transform should be enabled");

        let mut transformer = ChainedSourceFileTransformer::new(vec![
            Box::new(SourceFileTransformer::TypeEraser {
                compiler_options: compiler_options.clone(),
            }),
            Box::new(SourceFileTransformer::RuntimeSyntax {
                compiler_options: compiler_options.clone(),
                facts: crate::tstransforms::runtimesyntax::RuntimeSyntaxResolverFacts::default(),
            }),
            Box::new(SourceFileTransformer::ClassFields { config }),
            Box::new(SourceFileTransformer::UseStrict {
                compiler_options: compiler_options.clone(),
                file_module_format: core::ModuleKind::CommonJS,
            }),
            Box::new(SourceFileTransformer::CommonJsModule {
                compiler_options,
                facts: crate::moduletransforms::ModuleTransformFacts::default(),
            }),
        ]);
        let mut emit_context = printer::new_emit_context();
        emit_context.activate();

        let output =
            transform_at_public_boundary(&mut transformer, &source_file, &mut emit_context);
        let counts = finish_instrumentation_snapshot();
        let emitted = emit_source_file(&output, emit_context);

        assert!(emitted.contains("constructor()"), "{emitted}");
        assert!(emitted.contains("this.type = "), "{emitted}");
        assert!(!emitted.contains("\n    type = "), "{emitted}");
        assert_finishes_once(counts);
    }

    #[test]
    fn class_fields_transform_visits_nested_class_in_heritage_expression() {
        reset_finish_instrumentation();
        let source_file = parse_typescript(
            "export class Base {}\nexport class C extends Mixin([Base], (base: any) => { class Inner extends base { num: number = 0 } return Inner; }) {}",
        );
        assert!(
            source_file.diagnostics().is_empty(),
            "unexpected parse diagnostics"
        );
        let compiler_options = core::CompilerOptions {
            target: core::ScriptTarget::ES2015,
            module: core::ModuleKind::CommonJS,
            ..Default::default()
        };
        let config = crate::estransforms::classfields::class_field_transform_config(
            compiler_options.get_emit_script_target(),
            compiler_options.get_use_define_for_class_fields(),
            compiler_options.experimental_decorators.is_true(),
        )
        .expect("ES2015 class field transform should be enabled");

        let mut transformer = ChainedSourceFileTransformer::new(vec![
            Box::new(SourceFileTransformer::TypeEraser {
                compiler_options: compiler_options.clone(),
            }),
            Box::new(SourceFileTransformer::ClassFields { config }),
            Box::new(SourceFileTransformer::UseStrict {
                compiler_options: compiler_options.clone(),
                file_module_format: core::ModuleKind::CommonJS,
            }),
            Box::new(SourceFileTransformer::CommonJsModule {
                compiler_options: compiler_options.clone(),
                facts: crate::moduletransforms::ModuleTransformFacts::default(),
            }),
        ]);
        let mut emit_context = printer::new_emit_context();
        emit_context.activate();

        let output =
            transform_at_public_boundary(&mut transformer, &source_file, &mut emit_context);
        let counts = finish_instrumentation_snapshot();
        let emitted = emit_source_file(&output, emit_context);

        assert!(emitted.contains("constructor()"), "{emitted}");
        assert!(emitted.contains("this.num = 0;"), "{emitted}");
        assert!(!emitted.contains("\n        num = 0;"), "{emitted}");
        assert_finishes_once(counts);
    }

    #[test]
    fn type_script_transform_chain_lowers_export_equals_namespace_merge() {
        reset_finish_instrumentation();
        let source_file =
            parse_typescript("class foo {}\nnamespace foo { export var v = 1; }\nexport = foo;");
        assert!(
            source_file.diagnostics().is_empty(),
            "unexpected parse diagnostics"
        );
        let compiler_options = core::CompilerOptions {
            target: core::ScriptTarget::ES2015,
            module: core::ModuleKind::CommonJS,
            ..Default::default()
        };

        let mut transformer = ChainedSourceFileTransformer::new(vec![
            Box::new(SourceFileTransformer::TypeEraser {
                compiler_options: compiler_options.clone(),
            }),
            Box::new(SourceFileTransformer::RuntimeSyntax {
                compiler_options: compiler_options.clone(),
                facts: crate::tstransforms::runtimesyntax::RuntimeSyntaxResolverFacts::default(),
            }),
            Box::new(SourceFileTransformer::UseStrict {
                compiler_options: compiler_options.clone(),
                file_module_format: core::ModuleKind::CommonJS,
            }),
            Box::new(SourceFileTransformer::CommonJsModule {
                compiler_options,
                facts: crate::moduletransforms::ModuleTransformFacts::default(),
            }),
        ]);
        let mut emit_context = printer::new_emit_context();
        emit_context.activate();

        let output =
            transform_at_public_boundary(&mut transformer, &source_file, &mut emit_context);
        let counts = finish_instrumentation_snapshot();
        let emitted = emit_source_file(&output, emit_context);

        assert!(emitted.contains("(function (foo)"), "{emitted}");
        assert!(emitted.contains("module.exports = foo"), "{emitted}");
        assert!(!emitted.contains("namespace foo"), "{emitted}");
        assert_finishes_once(counts);
    }
}

pub mod importelision;
pub mod legacydecorators;
pub mod metadata;
pub mod runtimesyntax;
pub mod typeeraser;
#[cfg(test)]
pub mod typeeraser_test;
pub mod utilities;

use crate::{SourceFileTransformer, TransformOptions, Transformer};

pub fn new_metadata_transformer(opts: &TransformOptions) -> Transformer {
    let mut tx = Transformer::default();
    tx.new_source_file_transformer(
        SourceFileTransformer::Metadata {
            compiler_options: opts.compiler_options.clone(),
            facts: opts
                .metadata_facts
                .clone()
                .expect("MetadataTransformer requires resolver facts"),
        },
        Some(opts.context.fork()),
    );
    tx
}

pub fn new_type_eraser_transformer(opts: &TransformOptions) -> Transformer {
    let mut tx = Transformer::default();
    tx.new_source_file_transformer(
        SourceFileTransformer::TypeEraser {
            compiler_options: opts.compiler_options.clone(),
        },
        Some(opts.context.fork()),
    );
    tx
}

pub fn new_import_elision_transformer(opts: &TransformOptions) -> Transformer {
    if opts.compiler_options.verbatim_module_syntax.is_true() {
        panic!("ImportElisionTransformer should not be used with VerbatimModuleSyntax");
    }

    let mut tx = Transformer::default();
    tx.new_source_file_transformer(
        SourceFileTransformer::ImportElision {
            facts: opts
                .import_elision_facts
                .clone()
                .expect("ImportElisionTransformer requires resolver facts"),
        },
        Some(opts.context.fork()),
    );
    tx
}

pub fn new_runtime_syntax_transformer(opts: &TransformOptions) -> Transformer {
    let mut tx = Transformer::default();
    tx.new_source_file_transformer(
        SourceFileTransformer::RuntimeSyntax {
            compiler_options: opts.compiler_options.clone(),
            facts: opts
                .runtime_syntax_facts
                .clone()
                .expect("RuntimeSyntaxTransformer requires resolver facts"),
        },
        Some(opts.context.fork()),
    );
    tx
}

pub fn new_legacy_decorators_transformer(opts: &TransformOptions) -> Transformer {
    let mut tx = Transformer::default();
    tx.new_source_file_transformer(
        SourceFileTransformer::LegacyDecorators {
            compiler_options: opts.compiler_options.clone(),
            facts: opts
                .legacy_decorators_facts
                .clone()
                .expect("LegacyDecoratorsTransformer requires resolver facts"),
        },
        Some(opts.context.fork()),
    );
    tx
}

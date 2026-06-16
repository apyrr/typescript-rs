pub mod r#async;
pub mod classfields;
pub mod classthis;
pub mod definitions;
pub mod esdecorator;
pub mod exponentiation;
pub mod forawait;
pub mod logicalassignment;
pub mod nameevaluation;
pub mod nullishcoalescing;
pub mod objectrestspread;
pub mod optionalcatch;
pub mod optionalchain;
pub mod taggedtemplate;
pub mod usestrict;
pub mod using;
pub mod utilities;

use ts_core::ScriptTarget;

use crate::{SourceFileTransformer, TransformOptions, Transformer};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EsTransformerStep {
    UsingDeclaration,
    EsDecorator,
    ClassFields,
    LogicalAssignment,
    NullishCoalescing,
    OptionalChain,
    OptionalCatch,
    ObjectRestSpread,
    ForAwait,
    TaggedTemplateLiftRestriction,
    Async,
    Exponentiation,
}

pub fn es_transformer_steps_for_target(target: ScriptTarget) -> &'static [EsTransformerStep] {
    use EsTransformerStep::*;

    match target {
        ScriptTarget::ESNext => &[EsDecorator, ClassFields],
        // 2025: only module system syntax (import attributes, json modules), untransformed regex modifiers
        // 2024: no new downlevel syntax
        // 2023: no new downlevel syntax
        // 2022: class static blocks and class fields are handled by newClassFieldsTransformer
        ScriptTarget::ES2025
        | ScriptTarget::ES2024
        | ScriptTarget::ES2023
        | ScriptTarget::ES2022
        | ScriptTarget::ES2021 => &[UsingDeclaration, EsDecorator, ClassFields],
        ScriptTarget::ES2020 => &[
            UsingDeclaration,
            EsDecorator,
            ClassFields,
            LogicalAssignment,
        ],
        ScriptTarget::ES2019 => &[
            UsingDeclaration,
            EsDecorator,
            ClassFields,
            LogicalAssignment,
            NullishCoalescing,
            OptionalChain,
        ],
        ScriptTarget::ES2018 => &[
            UsingDeclaration,
            EsDecorator,
            ClassFields,
            LogicalAssignment,
            NullishCoalescing,
            OptionalChain,
            OptionalCatch,
        ],
        ScriptTarget::ES2017 => &[
            UsingDeclaration,
            EsDecorator,
            ClassFields,
            LogicalAssignment,
            NullishCoalescing,
            OptionalChain,
            OptionalCatch,
            ObjectRestSpread,
            ForAwait,
            TaggedTemplateLiftRestriction,
        ],
        ScriptTarget::ES2016 => &[
            UsingDeclaration,
            EsDecorator,
            ClassFields,
            LogicalAssignment,
            NullishCoalescing,
            OptionalChain,
            OptionalCatch,
            ObjectRestSpread,
            ForAwait,
            TaggedTemplateLiftRestriction,
            Async,
        ],
        _ => &[
            UsingDeclaration,
            EsDecorator,
            ClassFields,
            LogicalAssignment,
            NullishCoalescing,
            OptionalChain,
            OptionalCatch,
            ObjectRestSpread,
            ForAwait,
            TaggedTemplateLiftRestriction,
            Async,
            Exponentiation,
        ],
    }
}

pub fn get_es_transformer(opts: &TransformOptions) -> Option<Transformer> {
    let steps = es_transformer_steps_for_target(opts.compiler_options.get_emit_script_target());
    let mut constructed = Vec::new();

    for step in steps {
        match step {
            EsTransformerStep::UsingDeclaration => {
                let mut tx = Transformer::default();
                tx.new_source_file_transformer(
                    SourceFileTransformer::UsingDeclaration,
                    Some(opts.context.fork()),
                );
                constructed.push(tx);
            }
            EsTransformerStep::EsDecorator => {
                if esdecorator::new_es_decorator_transformer_enabled(
                    opts.compiler_options.experimental_decorators.is_true(),
                    opts.compiler_options.get_emit_script_target(),
                    opts.compiler_options.get_use_define_for_class_fields(),
                ) {
                    let mut tx = Transformer::default();
                    tx.new_source_file_transformer(
                        SourceFileTransformer::EsDecorator {
                            compiler_options: opts.compiler_options.clone(),
                        },
                        Some(opts.context.fork()),
                    );
                    constructed.push(tx);
                }
            }
            EsTransformerStep::ClassFields => {
                if let Some(config) = classfields::class_field_transform_config(
                    opts.compiler_options.get_emit_script_target(),
                    opts.compiler_options.get_use_define_for_class_fields(),
                    opts.compiler_options.experimental_decorators.is_true(),
                ) {
                    let mut tx = Transformer::default();
                    tx.new_source_file_transformer(
                        SourceFileTransformer::ClassFields { config },
                        Some(opts.context.fork()),
                    );
                    constructed.push(tx);
                }
            }
            EsTransformerStep::LogicalAssignment => {
                let mut tx = Transformer::default();
                tx.new_source_file_transformer(
                    SourceFileTransformer::LogicalAssignment,
                    Some(opts.context.fork()),
                );
                constructed.push(tx);
            }
            EsTransformerStep::OptionalChain => {
                let mut tx = Transformer::default();
                tx.new_source_file_transformer(
                    SourceFileTransformer::OptionalChain,
                    Some(opts.context.fork()),
                );
                constructed.push(tx);
            }
            EsTransformerStep::OptionalCatch => {
                let mut tx = Transformer::default();
                tx.new_source_file_transformer(
                    SourceFileTransformer::OptionalCatch,
                    Some(opts.context.fork()),
                );
                constructed.push(tx);
            }
            EsTransformerStep::NullishCoalescing => {
                let mut tx = Transformer::default();
                tx.new_source_file_transformer(
                    SourceFileTransformer::NullishCoalescing,
                    Some(opts.context.fork()),
                );
                constructed.push(tx);
            }
            EsTransformerStep::ObjectRestSpread => {
                let mut tx = Transformer::default();
                tx.new_source_file_transformer(
                    SourceFileTransformer::ObjectRestSpread {
                        compiler_options: opts.compiler_options.clone(),
                    },
                    Some(opts.context.fork()),
                );
                constructed.push(tx);
            }
            EsTransformerStep::ForAwait => {
                let mut tx = Transformer::default();
                tx.new_source_file_transformer(
                    SourceFileTransformer::ForAwait,
                    Some(opts.context.fork()),
                );
                constructed.push(tx);
            }
            EsTransformerStep::TaggedTemplateLiftRestriction => {
                let mut tx = Transformer::default();
                tx.new_source_file_transformer(
                    SourceFileTransformer::TaggedTemplateLiftRestriction,
                    Some(opts.context.fork()),
                );
                constructed.push(tx);
            }
            EsTransformerStep::Async => {
                let mut tx = Transformer::default();
                tx.new_source_file_transformer(
                    SourceFileTransformer::Async,
                    Some(opts.context.fork()),
                );
                constructed.push(tx);
            }
            EsTransformerStep::Exponentiation => {
                let mut tx = Transformer::default();
                tx.new_source_file_transformer(
                    SourceFileTransformer::Exponentiation,
                    Some(opts.context.fork()),
                );
                constructed.push(tx);
            }
        }
    }

    crate::chain::chain_constructed(steps.len(), constructed, Some(opts.context.fork()))
}

pub fn new_use_strict_transformer(
    opts: &TransformOptions,
    file_module_format: ts_core::ModuleKind,
) -> Transformer {
    let mut tx = Transformer::default();
    tx.new_source_file_transformer(
        SourceFileTransformer::UseStrict {
            compiler_options: opts.compiler_options.clone(),
            file_module_format,
        },
        Some(opts.context.fork()),
    );
    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn es_transformer_steps_follow_go_definitions_order() {
        use EsTransformerStep::*;

        assert_eq!(
            es_transformer_steps_for_target(ScriptTarget::ESNext),
            &[EsDecorator, ClassFields]
        );
        assert_eq!(
            es_transformer_steps_for_target(ScriptTarget::ES2021),
            &[UsingDeclaration, EsDecorator, ClassFields]
        );
        assert_eq!(
            es_transformer_steps_for_target(ScriptTarget::ES2020),
            &[
                UsingDeclaration,
                EsDecorator,
                ClassFields,
                LogicalAssignment
            ]
        );
        assert_eq!(
            es_transformer_steps_for_target(ScriptTarget::ES2016),
            &[
                UsingDeclaration,
                EsDecorator,
                ClassFields,
                LogicalAssignment,
                NullishCoalescing,
                OptionalChain,
                OptionalCatch,
                ObjectRestSpread,
                ForAwait,
                TaggedTemplateLiftRestriction,
                Async
            ]
        );
        assert_eq!(
            es_transformer_steps_for_target(ScriptTarget::ES5),
            &[
                UsingDeclaration,
                EsDecorator,
                ClassFields,
                LogicalAssignment,
                NullishCoalescing,
                OptionalChain,
                OptionalCatch,
                ObjectRestSpread,
                ForAwait,
                TaggedTemplateLiftRestriction,
                Async,
                Exponentiation
            ]
        );
    }
}

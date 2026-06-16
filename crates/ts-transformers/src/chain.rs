use ts_printer as printer;

use crate::{ChainedSourceFileTransformer, TransformContext, TransformResult, Transformer};

pub type TransformerStep<T> = fn(T, &mut TransformContext) -> T;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChainBuildDecision {
    PanicEmptyInput,
    ReturnSingleInput,
    ReturnNone,
    ReturnSingleConstructed,
    ReturnChained,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChainVisitDecision {
    VisitSourceFile,
    PanicNonSourceFile,
}

pub fn chain_build_decision(input_count: usize, constructed_count: usize) -> ChainBuildDecision {
    match (input_count, constructed_count) {
        (0, _) => ChainBuildDecision::PanicEmptyInput,
        (1, _) => ChainBuildDecision::ReturnSingleInput,
        (_, 0) => ChainBuildDecision::ReturnNone,
        (_, 1) => ChainBuildDecision::ReturnSingleConstructed,
        _ => ChainBuildDecision::ReturnChained,
    }
}

pub fn chain_visit_decision(is_source_file: bool) -> ChainVisitDecision {
    if is_source_file {
        ChainVisitDecision::VisitSourceFile
    } else {
        ChainVisitDecision::PanicNonSourceFile
    }
}

pub fn chain<T>(mut node: T, transformers: &[TransformerStep<T>]) -> TransformResult<T> {
    let mut context = TransformContext::default();

    for transformer in transformers {
        node = transformer(node, &mut context);
    }

    TransformResult {
        node,
        diagnostics: context.diagnostics,
    }
}

pub fn chain_constructed(
    input_count: usize,
    constructed: Vec<Transformer>,
    emit_context: Option<printer::EmitContext>,
) -> Option<Transformer> {
    match chain_build_decision(input_count, constructed.len()) {
        ChainBuildDecision::PanicEmptyInput => {
            panic!("Expected some number of transforms to chain, but got none")
        }
        ChainBuildDecision::ReturnSingleInput | ChainBuildDecision::ReturnSingleConstructed => {
            constructed.into_iter().next()
        }
        ChainBuildDecision::ReturnNone => None,
        ChainBuildDecision::ReturnChained => {
            let components = constructed
                .into_iter()
                .map(Transformer::into_source_file_transformer)
                .collect();
            let mut transformer = Transformer::default();
            transformer.new_source_file_transformer(
                ChainedSourceFileTransformer::new(components),
                emit_context,
            );
            Some(transformer)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SourceFileTransformer;
    use ts_ast as ast;
    use ts_core as core;

    fn use_strict_transformer() -> Transformer {
        let mut transformer = Transformer::default();
        transformer.new_source_file_transformer(
            SourceFileTransformer::UseStrict {
                compiler_options: core::CompilerOptions::default(),
                file_module_format: core::ModuleKind::CommonJS,
            },
            None,
        );
        transformer
    }

    fn source_file() -> ast::SourceFile {
        let mut factory = ast::NodeFactory::default();
        let statements = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::new(),
        );
        let file = factory.new_source_file(
            ast::SourceFileParseOptions {
                file_name: "/chain.ts".to_string(),
                path: "/chain.ts".to_string(),
                ..Default::default()
            },
            String::new(),
            statements,
            None,
        );
        factory.finish_parsed_source_file(file, ast::ParsedSourceFileMetadata::default())
    }

    #[test]
    fn chain_constructed_filters_like_go_chain() {
        assert!(chain_constructed(2, Vec::new(), None).is_none());
        assert!(chain_constructed(1, vec![use_strict_transformer()], None).is_some());
        assert!(chain_constructed(2, vec![use_strict_transformer()], None).is_some());
    }

    #[test]
    #[should_panic(expected = "Expected some number of transforms to chain, but got none")]
    fn chain_constructed_panics_for_empty_input() {
        let _ = chain_constructed(0, Vec::new(), None);
    }

    #[test]
    fn chained_transform_runs_components_left_to_right() {
        let mut chained = chain_constructed(
            2,
            vec![use_strict_transformer(), use_strict_transformer()],
            None,
        )
        .expect("chained transformer");
        let result = chained.transform_source_file(&source_file());
        let statements = result.statements_view();
        assert_eq!(statements.len(), 1);
        let statement = statements.first().unwrap();
        let expression = result.store().expression(statement).unwrap();
        assert_eq!(result.store().text(expression), "use strict");
    }
}

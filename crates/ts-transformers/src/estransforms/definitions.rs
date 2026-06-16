use ts_core::ScriptTarget;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EsTransformerChain {
    EsDecoratorAndClassFields,
    EsNext,
    Es2021,
    Es2020,
    Es2019,
    Es2018,
    Es2017,
    Es2016,
}

pub fn es_transformer_chain_for_target(target: ScriptTarget) -> EsTransformerChain {
    match target {
        ScriptTarget::ESNext => EsTransformerChain::EsDecoratorAndClassFields,
        ScriptTarget::ES2025
        | ScriptTarget::ES2024
        | ScriptTarget::ES2023
        | ScriptTarget::ES2022
        | ScriptTarget::ES2021 => EsTransformerChain::EsNext,
        ScriptTarget::ES2020 => EsTransformerChain::Es2021,
        ScriptTarget::ES2019 => EsTransformerChain::Es2020,
        ScriptTarget::ES2018 => EsTransformerChain::Es2019,
        ScriptTarget::ES2017 => EsTransformerChain::Es2018,
        ScriptTarget::ES2016 => EsTransformerChain::Es2017,
        _ => EsTransformerChain::Es2016,
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EsTransformDefinitions;

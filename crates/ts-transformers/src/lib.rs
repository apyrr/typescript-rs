#![forbid(unsafe_code)]

pub mod chain;
#[expect(
    dead_code,
    reason = "ported declaration transform helpers are ahead of current callers"
)]
pub mod declarations;
pub mod destructuring;
#[expect(
    dead_code,
    unused_assignments,
    reason = "ported ES transform helpers are ahead of current callers"
)]
pub mod estransforms;
#[expect(
    dead_code,
    reason = "ported const-enum inliner helpers are ahead of current callers"
)]
pub mod inliners;
#[expect(
    dead_code,
    reason = "ported JSX transform helpers are ahead of current callers"
)]
pub mod jsxtransforms;
pub mod modifiervisitor;
#[expect(
    dead_code,
    reason = "ported module transform helpers are ahead of current callers"
)]
pub mod moduletransforms;
pub mod transformer;
#[expect(
    dead_code,
    reason = "ported TypeScript transform helpers are ahead of current callers"
)]
pub mod tstransforms;
pub mod utilities;

pub use transformer::{
    ChainedSourceFileTransformer, SourceFileTransform, SourceFileTransformer, TransformOptions,
    Transformer,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TransformContext {
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TransformResult<T> {
    pub node: T,
    pub diagnostics: Vec<String>,
}

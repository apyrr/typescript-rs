#![forbid(unsafe_code)]

pub use ts_ast as ast;
pub use ts_astnav as astnav;
pub use ts_core as core;
pub use ts_stringutil as stringutil;

pub mod api;
pub mod context;
pub mod indent;
pub mod lsutil;
pub mod rule;
pub mod rulecontext;
pub mod rules;
pub mod rulesmap;
pub mod scanner;
pub mod span;
pub mod util;

#[cfg(test)]
mod api_test;
#[cfg(test)]
mod comment_test;
#[cfg(test)]
mod format_test;
#[cfg(test)]
mod indent_getindentation_test;
#[cfg(test)]
mod indent_test;

pub use api::*;
pub use context::*;
pub use indent::*;
#[expect(
    ambiguous_glob_reexports,
    reason = "format API preserves upstream-style glob re-exports"
)]
pub use rule::*;
pub use rulecontext::*;
pub use rules::*;
pub use rulesmap::*;
pub use scanner::*;
pub use span::*;
pub use util::*;

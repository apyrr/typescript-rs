#![forbid(unsafe_code)]
// pseudochecker is a limited "checker" that returns pseudo-"types" of expressions - mostly those which trivially have type nodes

// TODO: Late binding/symbol merging?
// In strada, `expressionToTypeNode` used many `resolver` methods whose net effect was just
// calling `Checker.GetMergedSymbol` on a symbol when dealing with accessors. Right now those
// just use Node.Symbol, which will fail to pair up late-bound symbols. In theory, this is actually
// fine, since ID can't possibly know if `set [q1()](a){}` and `get [q2()](): T {}` are connected
// without performing real type checking, regardless, so it shouldn't matter. If anything, it might be
// OK to add a "dumb" late binder that can merge multiple `[a.b.c]: T` together, but not anything else.
// This is an area of active ~~feature-creep~~ development in ID output, prerequisite refactoring would include
// extracting the `mergeSymbol` core checker logic into a reusable component.

pub struct PseudoChecker {
    strict_null_checks: bool,
    exact_optional_property_types: bool,
}

impl PseudoChecker {
    pub fn strict_null_checks(&self) -> bool {
        self.strict_null_checks
    }

    pub fn exact_optional_property_types(&self) -> bool {
        self.exact_optional_property_types
    }
}

mod lookup;
mod r#type;

pub use lookup::is_in_const_context;
pub use r#type::*;

pub fn new_pseudo_checker(
    strict_null_checks: bool,
    exact_optional_property_types: bool,
) -> PseudoChecker {
    PseudoChecker {
        strict_null_checks,
        exact_optional_property_types,
    }
}

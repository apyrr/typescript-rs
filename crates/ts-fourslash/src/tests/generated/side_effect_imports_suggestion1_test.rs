#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_side_effect_imports_suggestion1() {
    let mut t = TestingT;
    run_test_side_effect_imports_suggestion1(&mut t);
}

fn run_test_side_effect_imports_suggestion1(t: &mut TestingT) {
    if should_skip_if_failing("TestSideEffectImportsSuggestion1") {
        return;
    }
    let content = r#"// @allowJs: true
// @noEmit: true
// @module: commonjs
// @noUncheckedSideEffectImports: true
// @filename: moduleA/a.js
import "b";
import "c";
// @filename: node_modules/b.ts
var a = 10;
// @filename: node_modules/c.js
exports.a = 10;
c = 10;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_suggestion_diagnostics(&[]);
    done();
}

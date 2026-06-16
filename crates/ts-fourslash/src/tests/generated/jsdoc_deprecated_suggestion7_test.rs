#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_deprecated_suggestion7() {
    let mut t = TestingT;
    run_test_jsdoc_deprecated_suggestion7(&mut t);
}

fn run_test_jsdoc_deprecated_suggestion7(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocDeprecated_suggestion7") {
        return;
    }
    let content = r"enum Direction {
    Left = -1,
    Right = 1,
}
type T = Direction.Left
/** @deprecated */
const x = 1
type x = string
var y: x = 'hi'";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_suggestion_diagnostics(&[]);
    done();
}

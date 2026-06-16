#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_unused_identifier_parameter1() {
    let mut t = TestingT;
    run_test_code_fix_unused_identifier_parameter1(&mut t);
}

fn run_test_code_fix_unused_identifier_parameter1(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixUnusedIdentifier_parameter1") {
        return;
    }
    let content = r"// @noUnusedLocals: true
// @noUnusedParameters: true
function g(a, b) { b; }
g(1, 2);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &vec!["Remove unused declaration for: 'a'".to_string()]);
    done();
}

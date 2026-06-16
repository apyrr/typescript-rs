#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_declare_function() {
    let mut t = TestingT;
    run_test_declare_function(&mut t);
}

fn run_test_declare_function(t: &mut TestingT) {
    if should_skip_if_failing("TestDeclareFunction") {
        return;
    }
    let content = r"// @filename: index.ts
declare function";
    let (f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_workspace_symbol(&[workspace_symbol_case("", vec![])]);
    done();
}

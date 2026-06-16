#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_incremental_resolve_constructor_declaration() {
    let mut t = TestingT;
    run_test_incremental_resolve_constructor_declaration(&mut t);
}

fn run_test_incremental_resolve_constructor_declaration(t: &mut TestingT) {
    if should_skip_if_failing("TestIncrementalResolveConstructorDeclaration") {
        return;
    }
    let content = r#"class c1 {
    private b: number;
    constructor(a: string) {
        this.b = a;
    }
}
var val = new c1("hello");
/*1*/val;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var val: c1", "");
    f.verify_number_of_errors_in_current_file(1);
    done();
}

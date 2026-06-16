#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_negative_replace_qualified_name_with_indexed_access_type01() {
    let mut t = TestingT;
    run_test_code_fix_negative_replace_qualified_name_with_indexed_access_type01(&mut t);
}

fn run_test_code_fix_negative_replace_qualified_name_with_indexed_access_type01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"namespace Container {
    export interface Foo {
        bar: string;
    }
}
const x: [|Container.Foo.bar|] = """#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &[]);
    done();
}

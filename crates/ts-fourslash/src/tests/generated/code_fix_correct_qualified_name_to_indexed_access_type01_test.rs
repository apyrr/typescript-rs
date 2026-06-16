#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_correct_qualified_name_to_indexed_access_type01() {
    let mut t = TestingT;
    run_test_code_fix_correct_qualified_name_to_indexed_access_type01(&mut t);
}

fn run_test_code_fix_correct_qualified_name_to_indexed_access_type01(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixCorrectQualifiedNameToIndexedAccessType01") {
        return;
    }
    let content = r#"export interface Foo {
  bar: string;
}
export const x: [|Foo.bar|] = """#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "Foo[\"bar\"]", false, 0, 0);
    done();
}

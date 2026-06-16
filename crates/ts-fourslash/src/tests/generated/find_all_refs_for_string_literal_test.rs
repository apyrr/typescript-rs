#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_string_literal() {
    let mut t = TestingT;
    run_test_find_all_refs_for_string_literal(&mut t);
}

fn run_test_find_all_refs_for_string_literal(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsForStringLiteral") {
        return;
    }
    let content = r#"// @filename: /a.ts
interface Foo {
    property: /**/"foo";
}
/**
 * @type {{ property: "foo"}}
 */
const obj: Foo = {
    property: "foo",
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}

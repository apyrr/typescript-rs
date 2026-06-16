#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_private_fields1() {
    let mut t = TestingT;
    run_test_rename_private_fields1(&mut t);
}

fn run_test_rename_private_fields1(t: &mut TestingT) {
    if should_skip_if_failing("TestRenamePrivateFields1") {
        return;
    }
    let content = r#"class Foo {
   [|[|{| "contextRangeIndex": 0 |}#foo|] = 1;|]

   getFoo() {
       return this.[|#foo|];
   }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "#foo");
    done();
}

#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_private_method() {
    let mut t = TestingT;
    run_test_rename_private_method(&mut t);
}

fn run_test_rename_private_method(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class Foo {
   [|[|{| "contextRangeIndex": 0 |}#foo|]() { }|]
   callFoo() {
       return this.[|#foo|]();
   }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        f.get_ranges_by_text("#foo")
            .into_iter()
            .map(Into::into)
            .collect(),
    );
    done();
}

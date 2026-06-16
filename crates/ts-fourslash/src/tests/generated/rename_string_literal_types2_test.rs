#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_string_literal_types2() {
    let mut t = TestingT;
    run_test_rename_string_literal_types2(&mut t);
}

fn run_test_rename_string_literal_types2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"type Foo = "[|a|]" | "b";

class C {
    p: Foo = "[|a|]";
    m() {
        if (this.p === "[|a|]") {}
        if ("[|a|]" === this.p) {}

        if (this.p !== "[|a|]") {}
        if ("[|a|]" !== this.p) {}

        if (this.p == "[|a|]") {}
        if ("[|a|]" == this.p) {}

        if (this.p != "[|a|]") {}
        if ("[|a|]" != this.p) {}
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "a");
    done();
}

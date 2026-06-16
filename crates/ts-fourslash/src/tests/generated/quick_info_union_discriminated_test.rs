#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_union_discriminated() {
    let mut t = TestingT;
    run_test_quick_info_union_discriminated(&mut t);
}

fn run_test_quick_info_union_discriminated(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoUnion_discriminated") {
        return;
    }
    let content = r#"// @Filename: quickInfoJsDocTags.ts
type U = A | B;

interface A {
    /** Kind A */
    kind: "a";
    /** Prop A */
    prop: number;
}

interface B {
    /** Kind B */
    kind: "b";
    /** Prop B */
    prop: string;
}

const u: U = {
    /*uKind*/kind: "a",
    /*uProp*/prop: 0,
}
const u2: U = {
    /*u2Kind*/kind: "bogus",
    /*u2Prop*/prop: 1,
};"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "uKind", "(property) A.kind: \"a\"", "Kind A");
    f.verify_quick_info_at(t, "uProp", "(property) A.prop: number", "Prop A");
    f.verify_quick_info_at(t, "u2Kind", "(property) kind: \"bogus\"", "");
    f.verify_quick_info_at(t, "u2Prop", "(property) prop: number", "");
    done();
}

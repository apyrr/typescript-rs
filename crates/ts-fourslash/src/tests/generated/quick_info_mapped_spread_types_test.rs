#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_mapped_spread_types() {
    let mut t = TestingT;
    run_test_quick_info_mapped_spread_types(&mut t);
}

fn run_test_quick_info_mapped_spread_types(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface Foo {
    /** Doc */
    bar: number;
}

const f: Foo = { bar: 0 };
f./*f*/bar;

const f2: { [TKey in keyof Foo]: string } = { bar: "0" };
f2./*f2*/bar;

const f3 = { ...f };
f3./*f3*/bar;

const f4 = { ...f2 };
f4./*f4*/bar;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "f");
    f.verify_quick_info_is(t, "(property) Foo.bar: number", "Doc");
    f.go_to_marker(t, "f2");
    f.verify_quick_info_is(t, "(property) bar: string", "Doc");
    f.go_to_marker(t, "f3");
    f.verify_quick_info_is(t, "(property) Foo.bar: number", "Doc");
    f.go_to_marker(t, "f4");
    f.verify_quick_info_is(t, "(property) bar: string", "Doc");
    done();
}

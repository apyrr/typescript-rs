#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_union_of_namespaces() {
    let mut t = TestingT;
    run_test_quick_info_union_of_namespaces(&mut t);
}

fn run_test_quick_info_union_of_namespaces(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"declare const x: typeof A | typeof B;
x./**/f;

namespace A {
    export function f() {}
}
namespace B {
    export function f() {}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "(method) f(): void", "");
    done();
}

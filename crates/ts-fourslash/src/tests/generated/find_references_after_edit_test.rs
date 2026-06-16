#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_references_after_edit() {
    let mut t = TestingT;
    run_test_find_references_after_edit(&mut t);
}

fn run_test_find_references_after_edit(t: &mut TestingT) {
    if should_skip_if_failing("TestFindReferencesAfterEdit") {
        return;
    }
    let content = r"// @Filename: a.ts
interface A {
    /*1*/foo: string;
}
// @Filename: b.ts
///<reference path='a.ts'/>
/**/
function foo(x: A) {
    x./*2*/foo
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    f.go_to_marker(t, "");
    f.insert(t, "\n");
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}

#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_shorthand_property() {
    let mut t = TestingT;
    run_test_quick_info_for_shorthand_property(&mut t);
}

fn run_test_quick_info_for_shorthand_property(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @strict: false
var name1 = undefined, id1 = undefined;
var /*obj1*/obj1 = {/*name1*/name1, /*id1*/id1};
var name2 = "Hello";
var id2 = 10000;
var /*obj2*/obj2 = {/*name2*/name2, /*id2*/id2};"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "obj1",
        "var obj1: {\n    name1: any;\n    id1: any;\n}",
        "",
    );
    f.verify_quick_info_at(t, "name1", "(property) name1: any", "");
    f.verify_quick_info_at(t, "id1", "(property) id1: any", "");
    f.verify_quick_info_at(
        t,
        "obj2",
        "var obj2: {\n    name2: string;\n    id2: number;\n}",
        "",
    );
    f.verify_quick_info_at(t, "name2", "(property) name2: string", "");
    f.verify_quick_info_at(t, "id2", "(property) id2: number", "");
    done();
}

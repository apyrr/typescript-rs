#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_property_contextually_typed_by_type_param01() {
    let mut t = TestingT;
    run_test_find_all_refs_property_contextually_typed_by_type_param01(&mut t);
}

fn run_test_find_all_refs_property_contextually_typed_by_type_param01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface IFoo {
    /*1*/a: string;
}
class C<T extends IFoo> {
    method() {
        var x: T = {
            a: ""
        };
        x.a;
    }
}


var x: IFoo = {
    a: "ss"
};"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}

#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_inherited_properties5() {
    let mut t = TestingT;
    run_test_references_for_inherited_properties5(&mut t);
}

fn run_test_references_for_inherited_properties5(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface interface1 extends interface1 {
   /*1*/doStuff(): void;
   /*2*/propName: string;
}
interface interface2 extends interface1 {
   doStuff(): void;
   propName: string;
}

var v: interface1;
v.propName;
v.doStuff();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}

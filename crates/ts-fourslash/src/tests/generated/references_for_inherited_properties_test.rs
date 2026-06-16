#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_inherited_properties() {
    let mut t = TestingT;
    run_test_references_for_inherited_properties(&mut t);
}

fn run_test_references_for_inherited_properties(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForInheritedProperties") {
        return;
    }
    let content = r"interface interface1 {
    /*1*/doStuff(): void;
}

interface interface2  extends interface1{
    /*2*/doStuff(): void;
}

class class1 implements interface2 {
    /*3*/doStuff() {

    }
}

class class2 extends class1 {

}

var v: class2;
v./*4*/doStuff();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
        ],
    );
    done();
}

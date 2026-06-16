#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_inherited_properties3() {
    let mut t = TestingT;
    run_test_find_all_refs_inherited_properties3(&mut t);
}

fn run_test_find_all_refs_inherited_properties3(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsInheritedProperties3") {
        return;
    }
    let content = r"class class1 extends class1 {
    [|/*0*/doStuff() { }|]
    [|/*1*/propName: string;|]
}
interface interface1 extends interface1 {
    [|/*2*/doStuff(): void;|]
    [|/*3*/propName: string;|]
}
class class2 extends class1 implements interface1 {
    [|/*4*/doStuff() { }|]
    [|/*5*/propName: string;|]
}

var v: class2;
v./*6*/doStuff();
v./*7*/propName;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "0".to_string(),
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "6".to_string(),
            "5".to_string(),
            "7".to_string(),
        ],
    );
    done();
}

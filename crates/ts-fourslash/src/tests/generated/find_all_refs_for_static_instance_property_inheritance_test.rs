#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_static_instance_property_inheritance() {
    let mut t = TestingT;
    run_test_find_all_refs_for_static_instance_property_inheritance(&mut t);
}

fn run_test_find_all_refs_for_static_instance_property_inheritance(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsForStaticInstancePropertyInheritance") {
        return;
    }
    let content = r"class X{
	/*0*/foo:any
}

class Y extends X{
	static /*1*/foo:any
}

class Z extends Y{
	static /*2*/foo:any
	/*3*/foo:any
}

const x = new X();
const y = new Y();
const z = new Z();
x./*4*/foo;
y./*5*/foo;
z./*6*/foo;
Y./*7*/foo;
Z./*8*/foo;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "0".to_string(),
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
            "8".to_string(),
        ],
    );
    done();
}

#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_statics_and_members_with_same_names() {
    let mut t = TestingT;
    run_test_references_for_statics_and_members_with_same_names(&mut t);
}

fn run_test_references_for_statics_and_members_with_same_names(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace FindRef4 {
	namespace MixedStaticsClassTest {
		export class Foo {
			/*1*/bar: Foo;
			/*2*/static /*3*/bar: Foo;

			/*4*/public /*5*/foo(): void {
			}
			/*6*/public static /*7*/foo(): void {
			}
		}
	}

	function test() {
		// instance function
		var x = new MixedStaticsClassTest.Foo();
		x./*8*/foo();
		x./*9*/bar;

		// static function
		MixedStaticsClassTest.Foo./*10*/foo();
		MixedStaticsClassTest.Foo./*11*/bar;
	}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
            "8".to_string(),
            "9".to_string(),
            "10".to_string(),
            "11".to_string(),
        ],
    );
    done();
}

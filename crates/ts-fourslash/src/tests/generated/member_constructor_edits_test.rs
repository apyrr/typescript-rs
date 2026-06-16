#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_member_constructor_edits() {
    let mut t = TestingT;
    run_test_member_constructor_edits(&mut t);
}

fn run_test_member_constructor_edits(t: &mut TestingT) {
    if should_skip_if_failing("TestMemberConstructorEdits") {
        return;
    }
    let content = r#" namespace M {
     export class A {
		 constructor(a: string) {}
         public m(n: number) {
             return 0;
         }
         public n() {
             return this.m(0);
         }
     }
     export class B extends A {
     	constructor(a: string) {
			super(a);
		}
		/*1*/
	 }
	 var a = new A("s");
	 var b = new B("s");
 }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.go_to_marker(t, "1");
    f.insert(t, "public m(n: number) { return 0; }");
    f.verify_no_errors();
    done();
}

#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_overrides() {
    let mut t = TestingT;
    run_test_references_for_overrides(&mut t);
}

fn run_test_references_for_overrides(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"namespace FindRef3 {
	namespace SimpleClassTest {
		export class Foo {
			public /*foo*/foo(): void {
			}
		}
		export class Bar extends Foo {
			public foo(): void {
			}
		}
	}

	namespace SimpleInterfaceTest {
		export interface IFoo {
			/*ifoo*/ifoo(): void;
		}
		export interface IBar extends IFoo {
			ifoo(): void;
		}
	}

	namespace SimpleClassInterfaceTest {
		export interface IFoo {
			/*icfoo*/icfoo(): void;
		}
		export class Bar implements IFoo {
			public icfoo(): void {
			}
		}
	}

	namespace Test {
		export interface IBase {
			/*field*/field: string;
			/*method*/method(): void;
		}

		export interface IBlah extends IBase {
			field: string;
		}

		export interface IBlah2 extends IBlah {
			field: string;
		}

		export interface IDerived extends IBlah2 {
			method(): void;
		}

		export class Bar implements IDerived {
			public field: string;
			public method(): void { }
		}

		export class BarBlah extends Bar {
			public field: string;
		}
	}

	function test() {
		var x = new SimpleClassTest.Bar();
		x.foo();

		var y: SimpleInterfaceTest.IBar = null;
		y.ifoo();

        var w: SimpleClassInterfaceTest.Bar = null;
        w.icfoo();

		var z = new Test.BarBlah();
		z.field = "";
        z.method();
	}
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "foo".to_string(),
            "ifoo".to_string(),
            "icfoo".to_string(),
            "field".to_string(),
            "method".to_string(),
        ],
    );
    done();
}

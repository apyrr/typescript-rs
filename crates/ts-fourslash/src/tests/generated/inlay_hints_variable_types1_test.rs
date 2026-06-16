#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_variable_types1() {
    let mut t = TestingT;
    run_test_inlay_hints_variable_types1(&mut t);
}

fn run_test_inlay_hints_variable_types1(t: &mut TestingT) {
    if should_skip_if_failing("TestInlayHintsVariableTypes1") {
        return;
    }
    let content = r#"class C {}
namespace N { export class Foo {} }
interface Foo {}
const a = "a";
const b = 1;
const c = true;
const d = {} as Foo;
const e = <Foo>{};
const f = {} as const;
const g = (({} as const));
const h = new C();
const i = new N.C();
const j = ((((new C()))));
const k = { a: 1, b: 1 };
const l = ((({ a: 1, b: 1 })));
 const m = () => 123;
 const n;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}

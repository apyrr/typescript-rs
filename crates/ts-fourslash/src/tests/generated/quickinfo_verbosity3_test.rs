#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity3() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity3(&mut t);
}

fn run_test_quickinfo_verbosity3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /a.ts
export type X = { x: number };
export function f(x: X): void {}
// @Filename: /b.ts
import { f } from "./a";
/*1*/f({ x: 1 });
// @Filename: file.tsx
// @jsx: preserve
// @noLib: true
 interface OptionProp {
     propx: 2
 }
 class Opt extends React.Component<OptionProp, {}> {
     render() {
         return <div>Hello</div>;
     }
 }
 const obj1: OptionProp = {
     propx: 2
 }
 let y1 = <Opt/*2*/ propx={2} />;
// @Filename: a.ts
 interface Foo/*3*/<T extends Date> {
     prop: T
 }
 class Bar/*4*/<T extends Date> implements Foo<T> {
     prop!: T
 }
// @Filename: c.ts
 class c5b { public foo() { } }
 namespace c5b/*5*/ { export var y = 2; }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("1".to_string(), vec![0, 1]),
            ("2".to_string(), vec![0, 1, 2]),
            ("3".to_string(), vec![0, 1]),
            ("4".to_string(), vec![0, 1, 2]),
            ("5".to_string(), vec![0, 1, 2]),
        ]),
    );
    done();
}

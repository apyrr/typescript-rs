#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_import() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_import(&mut t);
}

fn run_test_quickinfo_verbosity_import(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @module: esnext
// @filename: /0.ts
export type Apple = {
    a: number;
    b: string;
}
export const a: Apple = { a: 1, b: "2"};
export enum Color {
    Red,
    Green,
    Blue,
}
// @filename: /1.ts
import * as zero from "./0";
const b/*b*/ = zero;
// @filename: /2.ts
import { a/*a*/ } from "./0";
import { Color/*c*/ } from "./0";
// @filename: /3.ts
export default class {
    a: boolean;
}
// @filename: /4.ts
import Foo/*d*/ from "./3";
const f/*e*/ = new Foo/*f*/();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("b".to_string(), vec![0, 1, 2]),
            ("a".to_string(), vec![0, 1]),
            ("c".to_string(), vec![0, 1]),
            ("d".to_string(), vec![0]),
            ("e".to_string(), vec![0, 1]),
            ("f".to_string(), vec![0, 1]),
        ]),
    );
    done();
}

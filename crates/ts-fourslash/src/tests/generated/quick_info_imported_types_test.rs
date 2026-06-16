#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_imported_types() {
    let mut t = TestingT;
    run_test_quick_info_imported_types(&mut t);
}

fn run_test_quick_info_imported_types(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: quickInfoImportedTypes.ts
/** This is an interface */
export interface Foo {
    a?: number;
}
/** One or two */
export type Bar = 1 | 2
/** This is a class */
export class Baz<T extends {}> {
    public x: T = {} as T
}
// @Filename: two.ts
import { Foo, Bar, Baz } from './quickInfoImportedTypes';
let x: Foo/*1*/;
let y: Bar/*2*/<any>;
let z: Baz/*3*/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "(alias) interface Foo\nimport Foo",
        "This is an interface",
    );
    f.verify_quick_info_at(t, "2", "(alias) type Bar = 1 | 2\nimport Bar", "One or two");
    f.verify_quick_info_at(
        t,
        "3",
        "(alias) class Baz<T extends {}>\nimport Baz",
        "This is a class",
    );
    done();
}

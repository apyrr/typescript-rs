#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_underscore_typings02() {
    let mut t = TestingT;
    run_test_underscore_typings02(&mut t);
}

fn run_test_underscore_typings02(t: &mut TestingT) {
    if should_skip_if_failing("TestUnderscoreTypings02") {
        return;
    }
    let content = r"// @strict: false
// @module: CommonJS
interface Dictionary<T> {
    [x: string]: T;
}
export interface ChainedObject<T> {
    functions: ChainedArray<string>;
    omit(): ChainedObject<T>;
    clone(): ChainedObject<T>;
}
interface ChainedDictionary<T> extends ChainedObject<Dictionary<>> {
    foldl(): ChainedObject<T>;
    clone(): ChainedDictionary<T>;
}
export interface ChainedArray<T> extends ChainedObject<Array<T>> {
    groupBy(): ChainedDictionary<any[]>;
    groupBy(propertyName): ChainedDictionary<any[]>;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_position(t, 0);
    f.verify_number_of_errors_in_current_file(2);
    done();
}

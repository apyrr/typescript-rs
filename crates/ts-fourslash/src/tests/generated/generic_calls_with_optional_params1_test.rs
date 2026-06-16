#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_calls_with_optional_params1() {
    let mut t = TestingT;
    run_test_generic_calls_with_optional_params1(&mut t);
}

fn run_test_generic_calls_with_optional_params1(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericCallsWithOptionalParams1") {
        return;
    }
    let content = r#"class Collection<T> {
    public add(x: T) { }
}
interface Utils {
    fold<T, S>(c: Collection<T>, folder: (s: S, t: T) => T, init?: S): T;
}
var c = new Collection<string>();
var utils: Utils;
var /*1*/r = utils.fold(c, (s, t) => t, "");
var /*2*/r2 = utils.fold(c, (s, t) => t);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var r: string", "");
    f.verify_quick_info_at(t, "2", "var r2: string", "");
    done();
}

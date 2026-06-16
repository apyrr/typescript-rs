#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_allow_late_bound_symbols_overwrite_early_bound_symbols() {
    let mut t = TestingT;
    run_test_allow_late_bound_symbols_overwrite_early_bound_symbols(&mut t);
}

fn run_test_allow_late_bound_symbols_overwrite_early_bound_symbols(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"export {};
const prop = "abc";
function foo(): void {};
foo.abc = 10;
foo[prop] = 10;
interface T0 {
    [prop]: number;
    abc: number;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    done();
}

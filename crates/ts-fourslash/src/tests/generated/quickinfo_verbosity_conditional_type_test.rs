#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_conditional_type() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_conditional_type(&mut t);
}

fn run_test_quickinfo_verbosity_conditional_type(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickinfoVerbosityConditionalType") {
        return;
    }
    let content = r#"interface Apple {
    color: string;
    weight: number;
}
type StrInt = string | bigint;
type T1<T extends Apple | Apple[]> = T extends { color: string } ? "one apple" : StrInt;
function f<T extends Apple | Apple[]>(x: T1<T>): void {
    x/*x*/;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([("x".to_string(), vec![0, 1, 2])]),
    );
    done();
}

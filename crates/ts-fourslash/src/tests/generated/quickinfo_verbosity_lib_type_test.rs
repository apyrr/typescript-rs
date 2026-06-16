#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_lib_type() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_lib_type(&mut t);
}

fn run_test_quickinfo_verbosity_lib_type(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickinfoVerbosityLibType") {
        return;
    }
    let content = r#"// @lib: es5
interface Apple {
    color: string;
    size: number;
}
function f(): Promise<Apple> {
    return Promise.resolve({ color: "red", size: 5 });
}
const g/*g*/ = f;
const u/*u*/: Map<string, Apple> = new Map;
type Foo<T> = Promise/*p*/<T>;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("g".to_string(), vec![0, 1]),
            ("u".to_string(), vec![0, 1]),
            ("p".to_string(), vec![0]),
        ]),
    );
    done();
}

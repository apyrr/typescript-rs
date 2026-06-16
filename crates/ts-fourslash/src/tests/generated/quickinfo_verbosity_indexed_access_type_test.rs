#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_indexed_access_type() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_indexed_access_type(&mut t);
}

fn run_test_quickinfo_verbosity_indexed_access_type(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickinfoVerbosityIndexedAccessType") {
        return;
    }
    let content = r#"interface T2 {
	"string key": string;
	"number key": number;
	"any key": string | number | symbol;
}
type K2 = "string key" | "any key";
function fn2<T extends T2>(obj: T, key: keyof T) {
	const value/*v1*/: T[K2] = undefined as any;
}
function fn3<K extends keyof T2>(obj: T2, key: K) {
    const value/*v2*/: T2[K] = undefined as any;;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("v1".to_string(), vec![0, 1]),
            ("v2".to_string(), vec![0, 1]),
        ]),
    );
    done();
}

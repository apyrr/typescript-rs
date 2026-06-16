#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_index_type() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_index_type(&mut t);
}

fn run_test_quickinfo_verbosity_index_type(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickinfoVerbosityIndexType") {
        return;
    }
    let content = r#"interface T1 {
	banana: string;
	grape: number;
	apple: boolean;
}
const x1/*x1*/: keyof T1 = 'banana';
const x2/*x2*/: keyof T1 & ("grape" | "apple") = 'grape';
function fn1<T extends T1>(obj: T, key: keyof T, k2: keyof T1) {
	if (key === k2/*k2*/) {
		return obj[key/*key*/];
	}
	return key;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("x1".to_string(), vec![0, 1]),
            ("x2".to_string(), vec![0]),
            ("k2".to_string(), vec![0, 1]),
            ("key".to_string(), vec![0]),
        ]),
    );
    done();
}

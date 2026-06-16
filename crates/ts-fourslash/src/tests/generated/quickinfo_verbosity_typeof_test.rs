#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_typeof() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_typeof(&mut t);
}

fn run_test_quickinfo_verbosity_typeof(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface Apple {
    color: string;
    weight: number;
}
const a: Apple = { color: "red", weight: 150 };
const b/*b*/: typeof a = { color: "green", weight: 120 };
class Banana {
    length: number;
    constructor(length: number) {
        this.length = length;
    }
}
const c/*c*/: typeof Banana = Banana;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("b".to_string(), vec![0, 1]),
            ("c".to_string(), vec![0, 1]),
        ]),
    );
    done();
}

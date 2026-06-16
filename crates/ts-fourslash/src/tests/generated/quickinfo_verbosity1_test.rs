#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity1() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity1(&mut t);
}

fn run_test_quickinfo_verbosity1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"type FooType = string | number;
const foo/*a*/: FooType = 1;
type BarType = FooType | boolean;
const bar/*b*/: BarType = 1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("a".to_string(), vec![0, 1]),
            ("b".to_string(), vec![0, 1, 2]),
        ]),
    );
    done();
}

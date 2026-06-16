#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_index_signature() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_index_signature(&mut t);
}

fn run_test_quickinfo_verbosity_index_signature(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"type Key = string | number;
interface Apple {
    banana: number;
}
interface Foo {
    [a/*a*/: Key]: Apple;
}
const f/*f*/: Foo = {};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("a".to_string(), vec![0, 1]),
            ("f".to_string(), vec![0, 1, 2]),
        ]),
    );
    done();
}

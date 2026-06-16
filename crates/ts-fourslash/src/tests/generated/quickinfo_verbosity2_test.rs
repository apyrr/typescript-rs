#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity2() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity2(&mut t);
}

fn run_test_quickinfo_verbosity2(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickinfoVerbosity2") {
        return;
    }
    let content = r"type Str = string | {};
type FooType = Str | number;
type Sym = symbol | (() => void);
type BarType = Sym | boolean;
type BothType = FooType | BarType;
const both/*b*/: BothType = 1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([("b".to_string(), vec![0, 1, 2, 3])]),
    );
    done();
}

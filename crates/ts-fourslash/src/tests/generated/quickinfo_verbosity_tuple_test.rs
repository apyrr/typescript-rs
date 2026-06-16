#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_tuple() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_tuple(&mut t);
}

fn run_test_quickinfo_verbosity_tuple(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickinfoVerbosityTuple") {
        return;
    }
    let content = r#"interface Orange {
    color: string;
}
interface Apple {
    color: string;
    other: Orange;
}
type TwoFruits/*T*/ = [Orange, Apple];
const tf/*f*/: TwoFruits = [
    { color: "orange" },
    { color: "red", other: { color: "orange" } }
];
const tf2/*f2*/: [Orange, Apple] = [
    { color: "orange" },
    { color: "red", other: { color: "orange" } }
];
type ManyFruits/*m*/ = (Orange | Apple)[];
const mf/*mf*/: ManyFruits = [];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("T".to_string(), vec![0, 1, 2]),
            ("f".to_string(), vec![0, 1, 2, 3]),
            ("f2".to_string(), vec![0, 1, 2]),
            ("m".to_string(), vec![0, 1, 2]),
            ("mf".to_string(), vec![0, 1, 2, 3]),
        ]),
    );
    done();
}

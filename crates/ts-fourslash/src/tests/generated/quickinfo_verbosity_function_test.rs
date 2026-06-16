#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_function() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_function(&mut t);
}

fn run_test_quickinfo_verbosity_function(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickinfoVerbosityFunction") {
        return;
    }
    let content = r"interface Apple {
    color: string;
    size: number;
}
interface Orchard {
    takeOneApple(a: Apple): void;
    getApple(): Apple;
    getApple(size: number): Apple[];
}
const o/*o*/: Orchard = {} as any;
declare function isApple/*f*/(x: unknown): x is Apple;
type SomeType = {
    prop1: string;
}
function someFun(a: SomeType): SomeType {
    return a;
}
someFun/*s*/.what = 'what';";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("o".to_string(), vec![0, 1, 2]),
            ("f".to_string(), vec![0, 1]),
            ("s".to_string(), vec![0, 1]),
        ]),
    );
    done();
}

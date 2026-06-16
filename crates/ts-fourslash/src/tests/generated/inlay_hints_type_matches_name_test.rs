#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_type_matches_name() {
    let mut t = TestingT;
    run_test_inlay_hints_type_matches_name(&mut t);
}

fn run_test_inlay_hints_type_matches_name(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"type Client = {};
function getClient(): Client { return {}; };
const client = getClient();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}

#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_server() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_server(&mut t);
}

fn run_test_quickinfo_verbosity_server(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
type FooType = string | number
const foo/*a*/: FooType = 1";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([("a".to_string(), vec![0, 1])]),
    );
    done();
}

#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports20() {
    let mut t = TestingT;
    run_test_organize_imports20(&mut t);
}

fn run_test_organize_imports20(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"const a = 1;
const b = 1;
export { a };
export { b };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r"const a = 1;
const b = 1;
export { a, b };
",
        "source.organizeImports",
        None,
    );
    done();
}

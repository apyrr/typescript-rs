#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_insert_second_try_catch_block() {
    let mut t = TestingT;
    run_test_insert_second_try_catch_block(&mut t);
}

fn run_test_insert_second_try_catch_block(t: &mut TestingT) {
    if should_skip_if_failing("TestInsertSecondTryCatchBlock") {
        return;
    }
    let content = r"try {} catch(e) { }
/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "try {} catch(e) { }");
    done();
}

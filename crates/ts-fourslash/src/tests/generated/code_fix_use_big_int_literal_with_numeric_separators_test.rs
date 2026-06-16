#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_use_big_int_literal_with_numeric_separators() {
    let mut t = TestingT;
    run_test_code_fix_use_big_int_literal_with_numeric_separators(&mut t);
}

fn run_test_code_fix_use_big_int_literal_with_numeric_separators(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"6_402_373_705_728_000;  // 18! < 2 ** 53
0x16_BE_EC_CA_73_00_00; // 18! < 2 ** 53";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &vec!["useBigintLiteral".to_string()]);
    done();
}

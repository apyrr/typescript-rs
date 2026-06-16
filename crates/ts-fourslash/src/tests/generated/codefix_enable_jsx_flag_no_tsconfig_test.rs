#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_codefix_enable_jsx_flag_no_tsconfig() {
    let mut t = TestingT;
    run_test_codefix_enable_jsx_flag_no_tsconfig(&mut t);
}

fn run_test_codefix_enable_jsx_flag_no_tsconfig(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: /dir/a.tsx
export const Component = () => <></>";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/dir/a.tsx");
    f.verify_code_fix_not_available(t, &[]);
    done();
}

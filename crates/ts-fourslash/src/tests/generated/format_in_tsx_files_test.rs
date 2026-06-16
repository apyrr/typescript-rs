#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_in_tsx_files() {
    let mut t = TestingT;
    run_test_format_in_tsx_files(&mut t);
}

fn run_test_format_in_tsx_files(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatInTsxFiles") {
        return;
    }
    let content = r"//@Filename: file.tsx
interface I<T1, T2> {
    next: I</* */
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    done();
}

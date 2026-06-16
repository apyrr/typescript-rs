#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_regexes() {
    let mut t = TestingT;
    run_test_formatting_regexes(&mut t);
}

fn run_test_formatting_regexes(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingRegexes") {
        return;
    }
    let content = r"removeAllButLast(sortedTypes, undefinedType, /keepNullableType**/ true)/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, ";");
    f.verify_current_line_content(
        t,
        "removeAllButLast(sortedTypes, undefinedType, /keepNullableType**/ true);",
    );
    done();
}

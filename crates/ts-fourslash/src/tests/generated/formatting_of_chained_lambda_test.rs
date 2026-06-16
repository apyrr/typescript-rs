#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_of_chained_lambda() {
    let mut t = TestingT;
    run_test_formatting_of_chained_lambda(&mut t);
}

fn run_test_formatting_of_chained_lambda(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingOfChainedLambda") {
        return;
    }
    let content = r"var fn = (x: string) => ()=> alert(x)/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, ";");
    f.verify_current_line_content(t, "var fn = (x: string) => () => alert(x);");
    done();
}

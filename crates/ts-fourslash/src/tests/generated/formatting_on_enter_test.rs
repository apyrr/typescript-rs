#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_enter() {
    let mut t = TestingT;
    run_test_formatting_on_enter(&mut t);
}

fn run_test_formatting_on_enter(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingOnEnter") {
        return;
    }
    let content = r"class foo { }
class bar {/**/ }
// new line here";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert_line(t, "");
    f.verify_current_file_content(
        t,
        r"class foo { }
class bar {
}
// new line here",
    );
    done();
}

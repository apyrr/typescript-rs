#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_formatting_on_pasting() {
    let mut t = TestingT;
    run_test_auto_formatting_on_pasting(&mut t);
}

fn run_test_auto_formatting_on_pasting(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoFormattingOnPasting") {
        return;
    }
    let content = r"namespace TestModule {
/**/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.paste(
        t,
        " class TestClass{\nprivate   foo;\npublic testMethod( )\n{}\n}",
    );
    f.verify_current_file_content(
        t,
        r"namespace TestModule {
    class TestClass {
        private foo;
        public testMethod() { }
    }
}",
    );
    done();
}

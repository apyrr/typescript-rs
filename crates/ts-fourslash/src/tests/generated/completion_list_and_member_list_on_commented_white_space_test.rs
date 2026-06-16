#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_and_member_list_on_commented_white_space() {
    let mut t = TestingT;
    run_test_completion_list_and_member_list_on_commented_white_space(&mut t);
}

fn run_test_completion_list_and_member_list_on_commented_white_space(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListAndMemberListOnCommentedWhiteSpace") {
        return;
    }
    let content = r"namespace M {
  export class C { public pub = 0; private priv = 1; }
  export var V = 0;
}


var c = new M.C();

c. // test on c.

//Test for comment
//c. /**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("".to_string()), None);
    done();
}

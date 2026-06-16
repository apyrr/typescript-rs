#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_enum_members_accept_non_ascii_strings() {
    let mut t = TestingT;
    run_test_quick_info_enum_members_accept_non_ascii_strings(&mut t);
}

fn run_test_quick_info_enum_members_accept_non_ascii_strings(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"enum Demo {
    /*Emoji*/Emoji = '🍎',
    /*Hebrew*/Hebrew = 'תפוח',
    /*Chinese*/Chinese = '苹果',
    /*Japanese*/Japanese = 'りんご',
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "Emoji", "(enum member) Demo.Emoji = \"🍎\"", "");
    f.verify_quick_info_at(t, "Hebrew", "(enum member) Demo.Hebrew = \"תפוח\"", "");
    f.verify_quick_info_at(t, "Chinese", "(enum member) Demo.Chinese = \"苹果\"", "");
    f.verify_quick_info_at(
        t,
        "Japanese",
        "(enum member) Demo.Japanese = \"りんご\"",
        "",
    );
    done();
}

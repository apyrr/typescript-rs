#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_white_space_trimming2() {
    let mut t = TestingT;
    run_test_white_space_trimming2(&mut t);
}

fn run_test_white_space_trimming2(t: &mut TestingT) {
    if should_skip_if_failing("TestWhiteSpaceTrimming2") {
        return;
    }
    let content = r"let noSubTemplate = `/*    /*1*/`;
let templateHead = `/*    /*2*/${1 + 2}`;
let templateMiddle = `/*    ${1 + 2    /*3*/}`;
let templateTail = `/*    ${1 + 2}    /*4*/`;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.go_to_marker(t, "2");
    f.insert(t, "\n");
    f.go_to_marker(t, "3");
    f.insert(t, "\n");
    f.go_to_marker(t, "4");
    f.insert(t, "\n");
    f.verify_current_file_content(
        t,
        r"let noSubTemplate = `/*    
`;
let templateHead = `/*    
${1 + 2}`;
let templateMiddle = `/*    ${1 + 2
    }`;
let templateTail = `/*    ${1 + 2}    
`;",
    );
    done();
}

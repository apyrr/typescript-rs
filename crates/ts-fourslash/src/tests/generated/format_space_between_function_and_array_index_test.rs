#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_space_between_function_and_array_index() {
    let mut t = TestingT;
    run_test_format_space_between_function_and_array_index(&mut t);
}

fn run_test_format_space_between_function_and_array_index(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5

function test() {
    return [];
}

test() [0]
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"
function test() {
    return [];
}

test()[0]
",
    );
    done();
}

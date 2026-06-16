#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_object_literal_open_curly_newline_assignment() {
    let mut t = TestingT;
    run_test_formatting_object_literal_open_curly_newline_assignment(&mut t);
}

fn run_test_formatting_object_literal_open_curly_newline_assignment(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingObjectLiteralOpenCurlyNewlineAssignment") {
        return;
    }
    let content = r"
var obj = {};
obj =
{
    prop: 3
};
 
var obj2 = obj ||
{
    prop: 0
}
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"
var obj = {};
obj =
{
    prop: 3
};

var obj2 = obj ||
{
    prop: 0
}
",
    );
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .indent_multi_line_object_literal_beginning_on_blank_line = ts_core::TSTrue;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"
var obj = {};
obj =
    {
        prop: 3
    };

var obj2 = obj ||
    {
        prop: 0
    }
",
    );
    done();
}

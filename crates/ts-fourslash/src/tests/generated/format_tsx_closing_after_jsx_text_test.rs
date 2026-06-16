#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_tsx_closing_after_jsx_text() {
    let mut t = TestingT;
    run_test_format_tsx_closing_after_jsx_text(&mut t);
}

fn run_test_format_tsx_closing_after_jsx_text(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatTsxClosingAfterJsxText") {
        return;
    }
    let content = r"// @Filename: foo.tsx

const a = (
    <div>
        text
               </div>
)
const b = (
    <div>
        text
      twice
               </div>
)
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"
const a = (
    <div>
        text
    </div>
)
const b = (
    <div>
        text
        twice
    </div>
)
",
    );
    done();
}

#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_jsx_texts3() {
    let mut t = TestingT;
    run_test_formatting_jsx_texts3(&mut t);
}

fn run_test_formatting_jsx_texts3(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingJsxTexts3") {
        return;
    }
    let content = r#"//@Filename: file.tsx
function foo() {
const bar = "Oh no";

return (
<div>"{bar}"</div>
)
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r#"function foo() {
    const bar = "Oh no";

    return (
        <div>"{bar}"</div>
    )
}"#,
    );
    done();
}

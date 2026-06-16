#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_jsx_texts4() {
    let mut t = TestingT;
    run_test_formatting_jsx_texts4(&mut t);
}

fn run_test_formatting_jsx_texts4(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingJsxTexts4") {
        return;
    }
    let content = r#"//@Filename: file.tsx
function foo() {
const a = <ns: foobar   x : test1   x :test2="string"  x:test3={true?1:0}  />;

return a;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r#"function foo() {
    const a = <ns:foobar x:test1 x:test2="string" x:test3={true ? 1 : 0} />;

    return a;
}"#,
    );
    done();
}

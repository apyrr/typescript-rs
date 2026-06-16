#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_async_await() {
    let mut t = TestingT;
    run_test_format_async_await(&mut t);
}

fn run_test_format_async_await(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatAsyncAwait") {
        return;
    }
    let content = r#"async   function asyncFunction() {/*asyncKeyword*/
    await
/*awaitExpressionIndent*/
    Promise.resolve("await");/*awaitExpressionAutoformat*/
    return  await   Promise.resolve("completed");/*awaitKeyword*/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "asyncKeyword");
    f.verify_current_line_content(t, "async function asyncFunction() {");
    f.go_to_marker(t, "awaitExpressionIndent");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "awaitExpressionAutoformat");
    f.verify_current_line_content(t, "        Promise.resolve(\"await\");");
    f.go_to_marker(t, "awaitKeyword");
    f.verify_current_line_content(t, "    return await Promise.resolve(\"completed\");");
    done();
}

#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_function_and_constructor_type() {
    let mut t = TestingT;
    run_test_format_function_and_constructor_type(&mut t);
}

fn run_test_format_function_and_constructor_type(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatFunctionAndConstructorType") {
        return;
    }
    let content = r"function renderElement(
    element: Element,
    renderNode:
(/*funcAutoformat*/
    node: Node/*funcParamAutoformat*/
/*funcIndent*/
    ) => void,
newNode:
new(/*constrAutoformat*/
    name: string/*constrParamAutoformat*/
/*constrIndent*/
) => Node
): void {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "funcAutoformat");
    f.verify_current_line_content(t, "        (");
    f.go_to_marker(t, "funcParamAutoformat");
    f.verify_current_line_content(t, "            node: Node");
    f.go_to_marker(t, "funcIndent");
    f.verify_indentation(t, 12);
    f.go_to_marker(t, "constrAutoformat");
    f.verify_current_line_content(t, "        new (");
    f.go_to_marker(t, "constrParamAutoformat");
    f.verify_current_line_content(t, "            name: string");
    f.go_to_marker(t, "constrIndent");
    f.verify_indentation(t, 12);
    done();
}

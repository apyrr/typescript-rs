#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_object_literal() {
    let mut t = TestingT;
    run_test_formatting_on_object_literal(&mut t);
}

fn run_test_formatting_on_object_literal(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingOnObjectLiteral") {
        return;
    }
    let content = r#"var x = /*1*/{foo:/*2*/ 1,
bar: "tt",/*3*/
boo: /*4*/1 + 5}/*5*/;

var x2 = /*6*/{foo/*7*/: 1,
bar: /*8*/"tt",boo:1+5}/*9*/;

function Foo() {/*10*/
var typeICalc = {/*11*/
clear: {/*12*/
"()": [1, 2, 3]/*13*/
}/*14*/
}/*15*/
}/*16*/

// Rule for object literal members for the "value" of the memebr to follow the indent/*17*/
// of the member, i.e. the relative position of the value is maintained when the member/*18*/
// is indented./*19*/
var x2 = {/*20*/
  foo:/*21*/
3,/*22*/
          'bar':/*23*/
                    { a: 1, b : 2}/*24*/
};/*25*/

var x={    };/*26*/
var y = {};/*27*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "var x = {");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    foo: 1,");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    bar: \"tt\",");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "    boo: 1 + 5");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "};");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "var x2 = {");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "    foo: 1,");
    f.go_to_marker(t, "8");
    f.verify_current_line_content(t, "    bar: \"tt\", boo: 1 + 5");
    f.go_to_marker(t, "9");
    f.verify_current_line_content(t, "};");
    f.go_to_marker(t, "10");
    f.verify_current_line_content(t, "function Foo() {");
    f.go_to_marker(t, "11");
    f.verify_current_line_content(t, "    var typeICalc = {");
    f.go_to_marker(t, "12");
    f.verify_current_line_content(t, "        clear: {");
    f.go_to_marker(t, "13");
    f.verify_current_line_content(t, "            \"()\": [1, 2, 3]");
    f.go_to_marker(t, "14");
    f.verify_current_line_content(t, "        }");
    f.go_to_marker(t, "15");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "16");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "17");
    f.verify_current_line_content(
        t,
        "// Rule for object literal members for the \"value\" of the memebr to follow the indent",
    );
    f.go_to_marker(t, "18");
    f.verify_current_line_content(
        t,
        "// of the member, i.e. the relative position of the value is maintained when the member",
    );
    f.go_to_marker(t, "19");
    f.verify_current_line_content(t, "// is indented.");
    f.go_to_marker(t, "20");
    f.verify_current_line_content(t, "var x2 = {");
    f.go_to_marker(t, "21");
    f.verify_current_line_content(t, "    foo:");
    f.go_to_marker(t, "22");
    f.verify_current_line_content(t, "        3,");
    f.go_to_marker(t, "23");
    f.verify_current_line_content(t, "    'bar':");
    f.go_to_marker(t, "24");
    f.verify_current_line_content(t, "        { a: 1, b: 2 }");
    f.go_to_marker(t, "25");
    f.verify_current_line_content(t, "};");
    f.go_to_marker(t, "26");
    f.verify_current_line_content(t, "var x = {};");
    f.go_to_marker(t, "27");
    f.verify_current_line_content(t, "var y = {};");
    done();
}

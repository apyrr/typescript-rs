#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_variety() {
    let mut t = TestingT;
    run_test_formatting_on_variety(&mut t);
}

fn run_test_formatting_on_variety(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingOnVariety") {
        return;
    }
    let content = r"function f(a,b,c,d){/*1*/
for(var i=0;i<10;i++){/*2*/
var a=0;/*3*/
var b=a+a+a*a%a/2-1;/*4*/
b+=a;/*5*/
++b;/*6*/
f(a,b,c,d);/*7*/
if(1===1){/*8*/
var m=function(e,f){/*9*/
return e^f;/*10*/
}/*11*/
}/*12*/
}/*13*/
}/*14*/

for (var i = 0   ; i < this.foo(); i++) {/*15*/
}/*16*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "function f(a, b, c, d) {");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    for (var i = 0; i < 10; i++) {");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "        var a = 0;");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "        var b = a + a + a * a % a / 2 - 1;");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "        b += a;");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "        ++b;");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "        f(a, b, c, d);");
    f.go_to_marker(t, "8");
    f.verify_current_line_content(t, "        if (1 === 1) {");
    f.go_to_marker(t, "9");
    f.verify_current_line_content(t, "            var m = function(e, f) {");
    f.go_to_marker(t, "10");
    f.verify_current_line_content(t, "                return e ^ f;");
    f.go_to_marker(t, "11");
    f.verify_current_line_content(t, "            }");
    f.go_to_marker(t, "12");
    f.verify_current_line_content(t, "        }");
    f.go_to_marker(t, "13");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "14");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "15");
    f.verify_current_line_content(t, "for (var i = 0; i < this.foo(); i++) {");
    f.go_to_marker(t, "16");
    f.verify_current_line_content(t, "}");
    done();
}

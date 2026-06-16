#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_closing_bracket() {
    let mut t = TestingT;
    run_test_formatting_on_closing_bracket(&mut t);
}

fn run_test_formatting_on_closing_bracket(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingOnClosingBracket") {
        return;
    }
    let content = r"function f( ) {/*1*/
var     x = 3;/*2*/
    var z = 2   ;/*3*/
    a  = z  ++ - 2 *  x ;/*4*/
        for ( ; ; ) {/*5*/
    a+=(g +g)*a%t;/*6*/
        b --                          ;/*7*/
}/*8*/

    switch ( a  )/*9*/
    {
        case 1  :     {/*10*/
    a ++  ;/*11*/
        b--;/*12*/
    if(a===a)/*13*/
                return;/*14*/
    else/*15*/
        {
            for(a in b)/*16*/
                if(a!=a)/*17*/
    {
    for(a in b)/*18*/
            {
a++;/*19*/
        }/*20*/
                }/*21*/
    }/*22*/
        }/*23*/
    default:/*24*/
        break;/*25*/
    }/*26*/
}/*27*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .insert_space_after_semicolon_in_for_statements = ts_core::TSTrue;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "function f() {");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    var x = 3;");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    var z = 2;");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "    a = z++ - 2 * x;");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "    for (; ;) {");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "        a += (g + g) * a % t;");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "        b--;");
    f.go_to_marker(t, "8");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "9");
    f.verify_current_line_content(t, "    switch (a) {");
    f.go_to_marker(t, "10");
    f.verify_current_line_content(t, "        case 1: {");
    f.go_to_marker(t, "11");
    f.verify_current_line_content(t, "            a++;");
    f.go_to_marker(t, "12");
    f.verify_current_line_content(t, "            b--;");
    f.go_to_marker(t, "13");
    f.verify_current_line_content(t, "            if (a === a)");
    f.go_to_marker(t, "14");
    f.verify_current_line_content(t, "                return;");
    f.go_to_marker(t, "15");
    f.verify_current_line_content(t, "            else {");
    f.go_to_marker(t, "16");
    f.verify_current_line_content(t, "                for (a in b)");
    f.go_to_marker(t, "17");
    f.verify_current_line_content(t, "                    if (a != a) {");
    f.go_to_marker(t, "18");
    f.verify_current_line_content(t, "                        for (a in b) {");
    f.go_to_marker(t, "19");
    f.verify_current_line_content(t, "                            a++;");
    f.go_to_marker(t, "20");
    f.verify_current_line_content(t, "                        }");
    f.go_to_marker(t, "21");
    f.verify_current_line_content(t, "                    }");
    f.go_to_marker(t, "22");
    f.verify_current_line_content(t, "            }");
    f.go_to_marker(t, "23");
    f.verify_current_line_content(t, "        }");
    f.go_to_marker(t, "24");
    f.verify_current_line_content(t, "        default:");
    f.go_to_marker(t, "25");
    f.verify_current_line_content(t, "            break;");
    f.go_to_marker(t, "26");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "27");
    f.verify_current_line_content(t, "}");
    done();
}

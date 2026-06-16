#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_statements_with_no_semicolon() {
    let mut t = TestingT;
    run_test_formatting_on_statements_with_no_semicolon(&mut t);
}

fn run_test_formatting_on_statements_with_no_semicolon(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingOnStatementsWithNoSemicolon") {
        return;
    }
    let content = r"/*1*/do
     { var a/*2*/
/*3*/}   while (1)
/*4*/function f() {
/*5*/    var s = 1
/*6*/            }
/*7*/switch (t) {
/*8*/    case 1:
/*9*/{
/*10*/test
/*11*/}
/*12*/}
/*13*/do{do{do{}while(a!==b)}while(a!==b)}while(a!==b)
/*14*/do{
/*15*/do{
/*16*/do{
/*17*/}while(a!==b)
/*18*/}while(a!==b)
/*19*/}while(a!==b)
/*20*/for(var i=0;i<10;i++){
/*21*/for(var j=0;j<10;j++){
/*22*/j-=i
/*23*/}/*24*/}
/*25*/function foo() {
/*26*/try {
/*27*/x+=2
/*28*/}
/*29*/catch( e){
/*30*/x+=2
/*31*/}finally {
/*32*/x+=2
/*33*/}
/*34*/}
/*35*/do     { var a }   while (1)
    foo(function (file) {/*49*/
        return 0/*50*/
    }).then(function (doc) {/*51*/
        return 1/*52*/
    });/*53*/
/*54*/if(1)
/*55*/if(1)
/*56*/x++
/*57*/else
/*58*/if(1)
/*59*/x+=2
/*60*/else
/*61*/x+=2



/*62*/;
         do do do do/*63*/
                test;/*64*/
            while (0)/*65*/
         while (0)/*66*/
            while (0)/*67*/
         while (0)/*68*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "do {");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    var a");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "} while (1)");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "function f() {");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "    var s = 1");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "switch (t) {");
    f.go_to_marker(t, "8");
    f.verify_current_line_content(t, "    case 1:");
    f.go_to_marker(t, "9");
    f.verify_current_line_content(t, "        {");
    f.go_to_marker(t, "10");
    f.verify_current_line_content(t, "            test");
    f.go_to_marker(t, "11");
    f.verify_current_line_content(t, "        }");
    f.go_to_marker(t, "12");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "13");
    f.verify_current_line_content(
        t,
        "do { do { do { } while (a !== b) } while (a !== b) } while (a !== b)",
    );
    f.go_to_marker(t, "14");
    f.verify_current_line_content(t, "do {");
    f.go_to_marker(t, "15");
    f.verify_current_line_content(t, "    do {");
    f.go_to_marker(t, "16");
    f.verify_current_line_content(t, "        do {");
    f.go_to_marker(t, "17");
    f.verify_current_line_content(t, "        } while (a !== b)");
    f.go_to_marker(t, "18");
    f.verify_current_line_content(t, "    } while (a !== b)");
    f.go_to_marker(t, "19");
    f.verify_current_line_content(t, "} while (a !== b)");
    f.go_to_marker(t, "20");
    f.verify_current_line_content(t, "for (var i = 0; i < 10; i++) {");
    f.go_to_marker(t, "21");
    f.verify_current_line_content(t, "    for (var j = 0; j < 10; j++) {");
    f.go_to_marker(t, "22");
    f.verify_current_line_content(t, "        j -= i");
    f.go_to_marker(t, "23");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "24");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "25");
    f.verify_current_line_content(t, "function foo() {");
    f.go_to_marker(t, "26");
    f.verify_current_line_content(t, "    try {");
    f.go_to_marker(t, "27");
    f.verify_current_line_content(t, "        x += 2");
    f.go_to_marker(t, "28");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "29");
    f.verify_current_line_content(t, "    catch (e) {");
    f.go_to_marker(t, "30");
    f.verify_current_line_content(t, "        x += 2");
    f.go_to_marker(t, "31");
    f.verify_current_line_content(t, "    } finally {");
    f.go_to_marker(t, "32");
    f.verify_current_line_content(t, "        x += 2");
    f.go_to_marker(t, "33");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "34");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "35");
    f.verify_current_line_content(t, "do { var a } while (1)");
    f.go_to_marker(t, "49");
    f.verify_current_line_content(t, "foo(function(file) {");
    f.go_to_marker(t, "50");
    f.verify_current_line_content(t, "    return 0");
    f.go_to_marker(t, "51");
    f.verify_current_line_content(t, "}).then(function(doc) {");
    f.go_to_marker(t, "52");
    f.verify_current_line_content(t, "    return 1");
    f.go_to_marker(t, "53");
    f.verify_current_line_content(t, "});");
    f.go_to_marker(t, "54");
    f.verify_current_line_content(t, "if (1)");
    f.go_to_marker(t, "55");
    f.verify_current_line_content(t, "    if (1)");
    f.go_to_marker(t, "56");
    f.verify_current_line_content(t, "        x++");
    f.go_to_marker(t, "57");
    f.verify_current_line_content(t, "    else");
    f.go_to_marker(t, "58");
    f.verify_current_line_content(t, "        if (1)");
    f.go_to_marker(t, "59");
    f.verify_current_line_content(t, "            x += 2");
    f.go_to_marker(t, "60");
    f.verify_current_line_content(t, "        else");
    f.go_to_marker(t, "61");
    f.verify_current_line_content(t, "            x += 2");
    f.go_to_marker(t, "62");
    f.verify_current_line_content(t, "                ;");
    f.go_to_marker(t, "63");
    f.verify_current_line_content(t, "do do do do");
    f.go_to_marker(t, "64");
    f.verify_current_line_content(t, "    test;");
    f.go_to_marker(t, "65");
    f.verify_current_line_content(t, "while (0)");
    f.go_to_marker(t, "66");
    f.verify_current_line_content(t, "while (0)");
    f.go_to_marker(t, "67");
    f.verify_current_line_content(t, "while (0)");
    f.go_to_marker(t, "68");
    f.verify_current_line_content(t, "while (0)");
    done();
}

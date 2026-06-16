#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_decorators() {
    let mut t = TestingT;
    run_test_formatting_decorators(&mut t);
}

fn run_test_formatting_decorators(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"/*1*/        @    decorator1    
/*2*/            @        decorator2
/*3*/    @decorator3
/*4*/        @    decorator4    @            decorator5
/*5*/class C {
/*6*/            @    decorator6    
/*7*/                @        decorator7
/*8*/        @decorator8
/*9*/    method1() { }

/*10*/        @    decorator9    @            decorator10 @decorator11            method2() { }

    method3(
/*11*/                @    decorator12    
/*12*/                    @        decorator13
/*13*/            @decorator14
/*14*/        x) { }

    method4(
/*15*/            @    decorator15    @            decorator16 @decorator17             x) { }

/*16*/            @    decorator18    
/*17*/                @        decorator19
/*18*/        @decorator20    
/*19*/    ["computed1"]() { }

/*20*/        @    decorator21    @            decorator22 @decorator23            ["computed2"]() { }

/*21*/            @    decorator24    
/*22*/                @        decorator25
/*23*/        @decorator26
/*24*/    get accessor1() { }

/*25*/        @    decorator27    @            decorator28 @decorator29            get accessor2() { }

/*26*/            @    decorator30    
/*27*/                @        decorator31
/*28*/        @decorator32
/*29*/    property1;

/*30*/        @    decorator33    @            decorator34 @decorator35            property2;
/*31*/function test(@decorator36@decorator37 param) {};
/*32*/function test2(@decorator38()@decorator39()param) {};
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "@decorator1");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "@decorator2");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "@decorator3");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "@decorator4 @decorator5");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "class C {");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "    @decorator6");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "    @decorator7");
    f.go_to_marker(t, "8");
    f.verify_current_line_content(t, "    @decorator8");
    f.go_to_marker(t, "9");
    f.verify_current_line_content(t, "    method1() { }");
    f.go_to_marker(t, "10");
    f.verify_current_line_content(t, "    @decorator9 @decorator10 @decorator11 method2() { }");
    f.go_to_marker(t, "11");
    f.verify_current_line_content(t, "        @decorator12");
    f.go_to_marker(t, "12");
    f.verify_current_line_content(t, "        @decorator13");
    f.go_to_marker(t, "13");
    f.verify_current_line_content(t, "        @decorator14");
    f.go_to_marker(t, "14");
    f.verify_current_line_content(t, "        x) { }");
    f.go_to_marker(t, "15");
    f.verify_current_line_content(t, "        @decorator15 @decorator16 @decorator17 x) { }");
    f.go_to_marker(t, "16");
    f.verify_current_line_content(t, "    @decorator18");
    f.go_to_marker(t, "17");
    f.verify_current_line_content(t, "    @decorator19");
    f.go_to_marker(t, "18");
    f.verify_current_line_content(t, "    @decorator20");
    f.go_to_marker(t, "19");
    f.verify_current_line_content(t, "    [\"computed1\"]() { }");
    f.go_to_marker(t, "20");
    f.verify_current_line_content(
        t,
        "    @decorator21 @decorator22 @decorator23 [\"computed2\"]() { }",
    );
    f.go_to_marker(t, "21");
    f.verify_current_line_content(t, "    @decorator24");
    f.go_to_marker(t, "22");
    f.verify_current_line_content(t, "    @decorator25");
    f.go_to_marker(t, "23");
    f.verify_current_line_content(t, "    @decorator26");
    f.go_to_marker(t, "24");
    f.verify_current_line_content(t, "    get accessor1() { }");
    f.go_to_marker(t, "25");
    f.verify_current_line_content(
        t,
        "    @decorator27 @decorator28 @decorator29 get accessor2() { }",
    );
    f.go_to_marker(t, "26");
    f.verify_current_line_content(t, "    @decorator30");
    f.go_to_marker(t, "27");
    f.verify_current_line_content(t, "    @decorator31");
    f.go_to_marker(t, "28");
    f.verify_current_line_content(t, "    @decorator32");
    f.go_to_marker(t, "29");
    f.verify_current_line_content(t, "    property1;");
    f.go_to_marker(t, "30");
    f.verify_current_line_content(t, "    @decorator33 @decorator34 @decorator35 property2;");
    f.go_to_marker(t, "31");
    f.verify_current_line_content(t, "function test(@decorator36 @decorator37 param) { };");
    f.go_to_marker(t, "32");
    f.verify_current_line_content(
        t,
        "function test2(@decorator38() @decorator39() param) { };",
    );
    done();
}

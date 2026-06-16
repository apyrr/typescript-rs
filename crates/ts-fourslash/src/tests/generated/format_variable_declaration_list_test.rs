#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_variable_declaration_list() {
    let mut t = TestingT;
    run_test_format_variable_declaration_list(&mut t);
}

fn run_test_format_variable_declaration_list(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatVariableDeclarationList") {
        return;
    }
    let content = r"/*1*/var   fun1   =   function   (     )     {
/*2*/            var               x   =   'foo'             ,
/*3*/                z   =   'bar'           ;
/*4*/                return  x            ;
/*5*/},

/*6*/fun2   =   (                function        (   f               )   {
/*7*/            var   fun   =   function   (        )       {
/*8*/                        console         .  log             (           f     (  )  )       ;
/*9*/            },
/*10*/            x   =   'Foo'           ;
/*11*/                return   fun            ;
/*12*/}   (           fun1            )   )       ;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "var fun1 = function() {");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    var x = 'foo',");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "        z = 'bar';");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "    return x;");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "},");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "    fun2 = (function(f) {");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "        var fun = function() {");
    f.go_to_marker(t, "8");
    f.verify_current_line_content(t, "            console.log(f());");
    f.go_to_marker(t, "9");
    f.verify_current_line_content(t, "        },");
    f.go_to_marker(t, "10");
    f.verify_current_line_content(t, "            x = 'Foo';");
    f.go_to_marker(t, "11");
    f.verify_current_line_content(t, "        return fun;");
    f.go_to_marker(t, "12");
    f.verify_current_line_content(t, "    }(fun1));");
    done();
}

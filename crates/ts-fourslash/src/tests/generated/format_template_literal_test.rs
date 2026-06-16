#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_template_literal() {
    let mut t = TestingT;
    run_test_format_template_literal(&mut t);
}

fn run_test_format_template_literal(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"var x = ` + "`" + `sadasdasdasdasfegsfd
/*1*/rasdesgeryt35t35y35 e4 ergt er 35t 3535 ` + "`" + `;
var y = ` + "`" + `1${2}/*2*/3` + "`" + `;

/*formatStart*/
let z=    ` + "`" + `foo` + "`" + `;/*3*/
let w=  ` + "`" + `bar${3}` + "`" + `;/*4*/
String.raw
 ` + "`" + `template` + "`" + `;/*5*/
String.raw` + "`" + `foo` + "`" + `;/*6*/
String.raw  ` + "`" + `bar${3}` + "`" + `;/*7*/
` + "`" + `Write ${   JSON.stringify("")   } and ${    (765)   } and ${   346  }` + "`" + `;/*spaceInside*/
/*formatEnd*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.go_to_marker(t, "2");
    f.insert(t, "\n");
    f.verify_indentation(t, 0);
    f.verify_current_line_content(t, "3`;");
    f.format_selection(t, "formatStart", "formatEnd");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "let z = `foo`;");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "let w = `bar${3}`;");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "    `template`;");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "String.raw`foo`;");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "String.raw`bar${3}`;");
    f.go_to_marker(t, "spaceInside");
    f.verify_current_line_content(
        t,
        "`Write ${JSON.stringify(\"\")} and ${(765)} and ${346}`;",
    );
    done();
}

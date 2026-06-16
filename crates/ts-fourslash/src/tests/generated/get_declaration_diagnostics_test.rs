#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_declaration_diagnostics() {
    let mut t = TestingT;
    run_test_get_declaration_diagnostics(&mut t);
}

fn run_test_get_declaration_diagnostics(t: &mut TestingT) {
    if should_skip_if_failing("TestGetDeclarationDiagnostics") {
        return;
    }
    let content = r#"// @strict: false
// @declaration: true
// @outFile: true
// @Filename: inputFile1.ts
namespace m {
   export function foo() {
       class C implements I { private a; }
       interface I { }
       return C;
   }
} /*1*/
// @Filename: input2.ts
var x = "hello world"; /*2*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_number_of_errors_in_current_file(1);
    f.go_to_marker(t, "2");
    f.verify_number_of_errors_in_current_file(0);
    done();
}

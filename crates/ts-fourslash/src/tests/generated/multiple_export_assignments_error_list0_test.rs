#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_multiple_export_assignments_error_list0() {
    let mut t = TestingT;
    run_test_multiple_export_assignments_error_list0(&mut t);
}

fn run_test_multiple_export_assignments_error_list0(t: &mut TestingT) {
    if should_skip_if_failing("TestMultipleExportAssignmentsErrorList0") {
        return;
    }
    let content = r"interface connectModule {
    (res, req, next): void;
}
interface connectExport {
    use: (mod: connectModule) => connectExport;
    listen: (port: number) => void;
}
var server: {
    (): connectExport;
    test1: connectModule;
    test2(): connectModule;
};
export = server;
/*1*/export = connectExport;
 
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.disable_formatting();
    f.go_to_marker(t, "1");
    f.delete_at_caret(t, 24);
    f.go_to_marker(t, "1");
    f.insert(t, "export = connectExport;\n");
    done();
}

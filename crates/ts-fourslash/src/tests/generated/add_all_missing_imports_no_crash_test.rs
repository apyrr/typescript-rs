#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_add_all_missing_imports_no_crash() {
    let mut t = TestingT;
    run_test_add_all_missing_imports_no_crash(&mut t);
}

fn run_test_add_all_missing_imports_no_crash(t: &mut TestingT) {
    if should_skip_if_failing("TestAddAllMissingImportsNoCrash") {
        return;
    }
    let content = r"// @Filename: file1.ts
export interface Test1 {}
export interface Test2 {}
export interface Test3 {}
export interface Test4 {}
// @Filename: file2.ts
import { Test1, Test4 } from './file1';
interface Testing {
    test1: Test1;
    test2: Test2;
    test3: Test3;
    test4: Test4;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "file2.ts");
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingImport".to_string(),
            new_file_content: r"import { Test1, Test2, Test3, Test4 } from './file1';
interface Testing {
    test1: Test1;
    test2: Test2;
    test3: Test3;
    test4: Test4;
}"
            .to_string(),
        },
    );
    done();
}

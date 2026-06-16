#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_all2() {
    let mut t = TestingT;
    run_test_import_name_code_fix_all2(&mut t);
}

fn run_test_import_name_code_fix_all2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: /path.ts
export declare function join(): void;
// @Filename: /os.ts
export declare function homedir(): void;
// @Filename: /index.ts

join();
homedir();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/index.ts");
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingImport".to_string(),
            new_file_content: r#"import { homedir } from "./os";
import { join } from "./path";

join();
homedir();"#
                .to_string(),
        },
    );
    done();
}

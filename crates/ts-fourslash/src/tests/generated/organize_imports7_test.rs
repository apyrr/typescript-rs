#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports7() {
    let mut t = TestingT;
    run_test_organize_imports7(&mut t);
}

fn run_test_organize_imports7(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"import * as something from "path"; /**
 * some comment here
 * and there
 */
import * as somethingElse from "anotherpath";

something;
somethingElse;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"import * as somethingElse from "anotherpath";
import * as something from "path"; /**
 * some comment here
 * and there
 */

something;
somethingElse;"#,
        "source.organizeImports",
        None,
    );
    done();
}

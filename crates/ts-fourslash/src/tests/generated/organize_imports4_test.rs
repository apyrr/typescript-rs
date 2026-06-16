#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports4() {
    let mut t = TestingT;
    run_test_organize_imports4(&mut t);
}

fn run_test_organize_imports4(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImports4") {
        return;
    }
    let content = r#"import * as something from "path";/** 
 * some comment here
 * and there
 */
import * as somethingElse from "anotherpath";
import * as AnotherThing from "somepath";/** 
 * some comment here
 * and there
 */
import * as AnotherThingElse from "someotherpath";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(t, r"", "source.organizeImports", None);
    done();
}

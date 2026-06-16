#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports6() {
    let mut t = TestingT;
    run_test_organize_imports6(&mut t);
}

fn run_test_organize_imports6(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImports6") {
        return;
    }
    let content = r#"import * as something from "path"; /* small comment */ // single line one.
/* some comment here
* and there
*/
import * as somethingElse from "anotherpath";
import * as anotherThing from "someopath"; /* small comment */ // single line one.
/* some comment here
* and there
*/
import * as anotherThingElse from "someotherpath";

anotherThing;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"/* some comment here
* and there
*/
import * as anotherThing from "someopath"; /* small comment */ // single line one.
/* some comment here
* and there
*/

anotherThing;"#,
        "source.organizeImports",
        None,
    );
    done();
}

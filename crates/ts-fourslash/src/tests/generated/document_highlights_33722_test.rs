#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_document_highlights_33722() {
    let mut t = TestingT;
    run_test_document_highlights_33722(&mut t);
}

fn run_test_document_highlights_33722(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /y.ts
class Foo {
  private foo() {}
}

const f = () => new Foo();
export default f;
// @Filename: /x.ts
import y from "./y";

y().[|foo|]();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights_with_options(
        t,
        None,
        vec!["/x.ts".to_string()],
        vec![MarkerOrRangeOrName::Range(f.ranges()[0].clone())],
    );
    done();
}

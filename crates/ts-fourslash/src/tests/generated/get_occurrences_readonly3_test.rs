#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_readonly3() {
    let mut t = TestingT;
    run_test_get_occurrences_readonly3(&mut t);
}

fn run_test_get_occurrences_readonly3(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesReadonly3") {
        return;
    }
    let content = r#"class C {
  [|readonly|] prop: /**/readonly string[] = [];
  constructor([|readonly|] prop2: string) {
    class D {
      readonly prop: string = "";  
    }
  }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        f.ranges()
            .into_iter()
            .map(MarkerOrRangeOrName::Range)
            .collect(),
    );
    f.verify_baseline_document_highlights(t, None, vec![MarkerOrRangeOrName::Name("".to_string())]);
    done();
}

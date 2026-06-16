#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_inherited_properties10() {
    let mut t = TestingT;
    run_test_references_for_inherited_properties10(&mut t);
}

fn run_test_references_for_inherited_properties10(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface IFeedbackHandler {
  /*1*/handleAccept?(): void;
  handleReject?(): void;
}

abstract class AbstractFeedbackHandler implements IFeedbackHandler {}

class FeedbackHandler extends AbstractFeedbackHandler {
  /*2*/handleAccept(): void {
    console.log("Feedback accepted");
  }

  handleReject(): void {
    console.log("Feedback rejected");
  }
}

function foo(handler: IFeedbackHandler) {
  handler./*3*/handleAccept?.();
  handler.handleReject?.();
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}

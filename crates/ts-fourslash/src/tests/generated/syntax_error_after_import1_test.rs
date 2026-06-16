#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_syntax_error_after_import1() {
    let mut t = TestingT;
    run_test_syntax_error_after_import1(&mut t);
}

fn run_test_syntax_error_after_import1(t: &mut TestingT) {
    if should_skip_if_failing("TestSyntaxErrorAfterImport1") {
        return;
    }
    let content = r#"declare module "extmod" {
  namespace IntMod {
    class Customer {
      constructor(name: string);
    }
  }
}
import ext = require('extmod');
import int = ext.IntMod;
var x = new int/*0*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "0");
    f.insert(t, ".");
    done();
}

#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_outlining_spans_for_imports_and_exports() {
    let mut t = TestingT;
    run_test_outlining_spans_for_imports_and_exports(&mut t);
}

fn run_test_outlining_spans_for_imports_and_exports(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"import { a1, a2 } from "a";
;
import {
} from "a";
;
import [|{
  b1,
  b2,
}|] from "b";
;
import j1 from "./j" with { type: "json" };
;
import j2 from "./j" with {
};
;
import j3 from "./j" with [|{
  type: "json"
}|];
;
[|import { a5, a6 } from "a";
import [|{
  a7,
  a8,
}|] from "a";|]
export { a1, a2 };
;
export { a3, a4 } from "a";
;
export {
};
;
export [|{
  b1,
  b2,
}|];
;
export {
} from "b";
;
export [|{
  b3,
  b4,
}|] from "b";
;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}

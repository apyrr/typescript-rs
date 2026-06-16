#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_is_write_access() {
    let mut t = TestingT;
    run_test_get_occurrences_is_write_access(&mut t);
}

fn run_test_get_occurrences_is_write_access(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesIsWriteAccess") {
        return;
    }
    let content = r#"var [|{| "isWriteAccess": true |}x|] = 0;
var assignmentRightHandSide = [|{| "isWriteAccess": false |}x|];
var assignmentRightHandSide2 = 1 + [|{| "isWriteAccess": false |}x|];

[|{| "isWriteAccess": true |}x|] = 1;
[|{| "isWriteAccess": true |}x|] = [|{| "isWriteAccess": false |}x|] + [|{| "isWriteAccess": false |}x|];

[|{| "isWriteAccess": false |}x|] == 1;
[|{| "isWriteAccess": false |}x|] <= 1;

var preIncrement = ++[|{| "isWriteAccess": true |}x|];
var postIncrement = [|{| "isWriteAccess": true |}x|]++;
var preDecrement = --[|{| "isWriteAccess": true |}x|];
var postDecrement = [|{| "isWriteAccess": true |}x|]--;

[|{| "isWriteAccess": true |}x|] += 1;
[|{| "isWriteAccess": true |}x|] <<= 1;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        vec![MarkerOrRangeOrName::Range(f.ranges()[0].clone())],
    );
    done();
}

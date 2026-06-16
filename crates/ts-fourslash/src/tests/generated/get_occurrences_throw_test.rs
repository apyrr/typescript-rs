#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_throw() {
    let mut t = TestingT;
    run_test_get_occurrences_throw(&mut t);
}

fn run_test_get_occurrences_throw(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"function f(a: number) {
    try {
        throw "Hello";

        try {
            throw 10;
        }
        catch (x) {
            [|return|] 100;
        }
        finally {
            throw 10;
        }
    }
    catch (x) {
        [|throw|] "Something";
    }
    finally {
        [|throw|] "Also something";
    }
    if (a > 0) {
        [|return|] (function () {
            return;
            return;
            return;

            if (false) {
                return true;
            }
            throw "Hello!";
        })() || true;
    }

    [|th/**/row|] 10;

    var unusued = [1, 2, 3, 4].map(x => { throw 4 })

    [|return|];
    [|return|] true;
    [|throw|] false;
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
    done();
}

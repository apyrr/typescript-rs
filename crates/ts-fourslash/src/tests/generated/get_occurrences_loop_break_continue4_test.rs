#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_loop_break_continue4() {
    let mut t = TestingT;
    run_test_get_occurrences_loop_break_continue4(&mut t);
}

fn run_test_get_occurrences_loop_break_continue4(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesLoopBreakContinue4") {
        return;
    }
    let content = r"var arr = [1, 2, 3, 4];
label1: for (var n in arr) {
    break;
    continue;
    break label1;
    continue label1;

    label2: for (var i = 0; i < arr[n]; i++) {
        break label1;
        continue label1;

        break;
        continue;
        break label2;
        continue label2;

        function foo() {
            label3: while (true) {
                break;
                continue;
                break label3;
                continue label3;

                // these cross function boundaries
                break label1;
                continue label1;
                break label2;
                continue label2;

                label4: [|do|] {
                    [|break|];
                    [|continue|];
                    [|break|] label4;
                    [|continue|] label4;

                    break label3;
                    continue label3;

                    switch (10) {
                        case 1:
                        case 2:
                            break;
                            [|break|] label4;
                        default:
                            [|continue|];
                    }

                    // these cross function boundaries
                    break label1;
                    continue label1;
                    break label2;
                    continue label2;
                    () => { break; }
                } [|wh/**/ile|] (true)
            }
        }
    }
}

label5: while (true) break label5;

label7: while (true) continue label5;";
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

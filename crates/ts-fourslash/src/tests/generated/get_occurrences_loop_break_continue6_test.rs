#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_loop_break_continue6() {
    let mut t = TestingT;
    run_test_get_occurrences_loop_break_continue6(&mut t);
}

fn run_test_get_occurrences_loop_break_continue6(t: &mut TestingT) {
    skip_if_failing(t);
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
                br/*1*/eak label1;
                cont/*2*/inue label1;
                bre/*3*/ak label2;
                c/*4*/ontinue label2;

                label4: do {
                    break;
                    continue;
                    break label4;
                    continue label4;

                    break label3;
                    continue label3;

                    switch (10) {
                        case 1:
                        case 2:
                            break;
                            break label4;
                        default:
                            continue;
                    }

                    // these cross function boundaries
                    br/*5*/eak label1;
                    co/*6*/ntinue label1;
                    br/*7*/eak label2;
                    con/*8*/tinue label2;
                    () => { b/*9*/reak; }
                } while (true)
            }
        }
    }
}

label5: while (true) break label5;

label7: while (true) co/*10*/ntinue label5;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        f.markers()
            .into_iter()
            .map(MarkerOrRangeOrName::Marker)
            .collect(),
    );
    done();
}

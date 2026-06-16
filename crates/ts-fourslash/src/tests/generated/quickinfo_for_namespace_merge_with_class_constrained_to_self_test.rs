#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_for_namespace_merge_with_class_constrained_to_self() {
    let mut t = TestingT;
    run_test_quickinfo_for_namespace_merge_with_class_constrained_to_self(&mut t);
}

fn run_test_quickinfo_for_namespace_merge_with_class_constrained_to_self(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickinfoForNamespaceMergeWithClassConstrainedToSelf") {
        return;
    }
    let content = r"declare namespace AMap {
    namespace MassMarks {
        interface Data {
            style?: number;
        }
    }
    class MassMarks<D extends MassMarks.Data = MassMarks.Data> {
        constructor(data: D[] | string);
        clear(): void;
    }
}

interface MassMarksCustomData extends AMap.MassMarks./*1*/Data {
    name: string;
    id: string;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "interface AMap.MassMarks<D extends AMap.MassMarks.Data = AMap.MassMarks.Data>.Data",
        "",
    );
    done();
}

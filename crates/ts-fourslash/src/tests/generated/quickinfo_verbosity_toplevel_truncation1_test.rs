#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_toplevel_truncation1() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_toplevel_truncation1(&mut t);
}

fn run_test_quickinfo_verbosity_toplevel_truncation1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"export enum LargeEnum/*1*/ {
    Member1,
    Member2,
    Member3,
    Member4,
    Member5,
    Member6,
    Member7,
    Member8,
    Member9,
    Member10,
    Member11,
    Member12,
    Member13,
    Member14,
    Member15,
    Member16,
    Member17,
    Member18,
    Member19,
    Member20,
    Member21,
    Member22,
    Member23,
    Member24,
    Member25,
}
export interface LargeInterface/*2*/ {
    property1: string;
    property2: number;
    property3: boolean;
    property4: Date;
    property5: string[];
    property6: number[];
    property7: boolean[];
    property8: { [key: string]: unknown };
    property9: string | null;
    property10: number | null;
    property11: boolean | null;
    property12: Date | null;
    property13: string | number;
    property14: number | boolean;
    property15: string | boolean;
    property16: Array<{ id: number; name: string }>;
    property17: Array<{ key: string; value: unknown }>;
    property18: { nestedProp1: string; nestedProp2: number };
    property19: { nestedProp3: boolean; nestedProp4: Date };
    property20: () => void;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([("1".to_string(), vec![1]), ("2".to_string(), vec![1])]),
    );
    done();
}

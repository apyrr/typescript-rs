#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_import_of_re_export2() {
    let mut t = TestingT;
    run_test_rename_import_of_re_export2(&mut t);
}

fn run_test_rename_import_of_re_export2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"declare module "a" {
    [|export class /*1*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 0 |}C|] {}|]
}
declare module "b" {
    [|export { [|{| "contextRangeIndex": 2 |}C|] as /*2*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 2 |}D|] } from "a";|]
}
declare module "c" {
    [|import { /*3*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 5 |}D|] } from "b";|]
    export function f(c: [|D|]): void;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        f.get_ranges_by_text("C")
            .into_iter()
            .map(Into::into)
            .collect(),
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.get_ranges_by_text("D")[0].clone().into()],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.get_ranges_by_text("D")[1].clone().into(),
            f.get_ranges_by_text("D")[2].clone().into(),
        ],
    );
    done();
}

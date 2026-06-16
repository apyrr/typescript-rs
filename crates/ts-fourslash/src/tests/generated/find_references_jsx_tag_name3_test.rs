#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_references_jsx_tag_name3() {
    let mut t = TestingT;
    run_test_find_references_jsx_tag_name3(&mut t);
}

fn run_test_find_references_jsx_tag_name3(t: &mut TestingT) {
    if should_skip_if_failing("TestFindReferencesJSXTagName3") {
        return;
    }
    let content = r"// @jsx: preserve
// @Filename: /a.tsx
namespace JSX {
    export interface Element { }
    export interface IntrinsicElements {
        [|[|/*1*/div|]: any;|]
    }
}

[|const [|/*6*/Comp|] = () =>
    [|<[|/*2*/div|]>
        Some content
        [|<[|/*3*/div|]>More content</[|/*4*/div|]>|]
    </[|/*5*/div|]>|];|]

const x = [|<[|/*7*/Comp|]>
    Content
</[|/*8*/Comp|]>|];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
            "8".to_string(),
        ],
    );
    f.verify_baseline_document_highlights(
        t,
        None,
        vec![
            MarkerOrRangeOrName::Range(f.ranges()[1].clone()),
            MarkerOrRangeOrName::Range(f.ranges()[5].clone()),
            MarkerOrRangeOrName::Range(f.ranges()[7].clone()),
            MarkerOrRangeOrName::Range(f.ranges()[8].clone()),
            MarkerOrRangeOrName::Range(f.ranges()[9].clone()),
        ],
    );
    f.verify_baseline_document_highlights(
        t,
        None,
        vec![
            MarkerOrRangeOrName::Range(f.ranges()[3].clone()),
            MarkerOrRangeOrName::Range(f.ranges()[11].clone()),
            MarkerOrRangeOrName::Range(f.ranges()[12].clone()),
        ],
    );
    done();
}

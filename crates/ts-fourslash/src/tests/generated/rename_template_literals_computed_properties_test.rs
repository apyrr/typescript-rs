#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_template_literals_computed_properties() {
    let mut t = TestingT;
    run_test_rename_template_literals_computed_properties(&mut t);
}

fn run_test_rename_template_literals_computed_properties(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: a.ts
interface Obj {
    [|[` + "`" + `[|{| "contextRangeIndex": 0 |}num|]` + "`" + `]: number;|]
    [|['[|{| "contextRangeIndex": 2 |}bool|]']: boolean;|]
}

let o: Obj = {
    [|[` + "`" + `[|{| "contextRangeIndex": 4 |}num|]` + "`" + `]: 0|],
    [|['[|{| "contextRangeIndex": 6 |}bool|]']: true|],
};

o = {
    [|['[|{| "contextRangeIndex": 8 |}num|]']: 1|],
    [|[` + "`" + `[|{| "contextRangeIndex": 10 |}bool|]` + "`" + `]: false|],
};

o.[|num|];
o['[|num|]'];
o["[|num|]"];
o[` + "`" + `[|num|]` + "`" + `];

o.[|bool|];
o['[|bool|]'];
o["[|bool|]"];
o[` + "`" + `[|bool|]` + "`" + `];

export { o };
// @allowJs: true
// @Filename: b.js
import { o as obj } from './a';

obj.[|num|];
obj[` + "`" + `[|num|]` + "`" + `];

obj.[|bool|];
obj[` + "`" + `[|bool|]` + "`" + `];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "num");
    done();
}

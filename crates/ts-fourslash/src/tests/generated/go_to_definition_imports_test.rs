#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_imports() {
    let mut t = TestingT;
    run_test_go_to_definition_imports(&mut t);
}

fn run_test_go_to_definition_imports(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /a.ts
export default function /*fDef*/f() {}
export const /*xDef*/x = 0;
// @Filename: /b.ts
/*bDef*/declare const b: number;
export = b;
// @Filename: /b.ts
import f, { x } from "./a";
import * as /*aDef*/a from "./a";
import b = require("./b");
[|/*fUse*/f|];
[|/*xUse*/x|];
[|/*aUse*/a|];
[|/*bUse*/b|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "aUse".to_string(),
            "fUse".to_string(),
            "xUse".to_string(),
            "bUse".to_string(),
        ],
    );
    done();
}

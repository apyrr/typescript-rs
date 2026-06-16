#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_default_export09() {
    let mut t = TestingT;
    run_test_find_all_refs_for_default_export09(&mut t);
}

fn run_test_find_all_refs_for_default_export09(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @filename: /tsconfig.json
{
    "compilerOptions": {
        "target": "esnext",
        "strict": true,
        "outDir": "./out",
        "allowSyntheticDefaultImports": true
    }
}
// @filename: /a.js
module.exports = [];
// @filename: /b.js
module.exports = 1;
// @filename: /c.ts
export = [];
// @filename: /d.ts
export = 1;
// @filename: /foo.ts
import * as /*0*/a from "./a.js"
import /*1*/aDefault from "./a.js"
import * as /*2*/b from "./b.js"
import /*3*/bDefault from "./b.js"

import * as /*4*/c from "./c"
import /*5*/cDefault from "./c"
import * as /*6*/d from "./d"
import /*7*/dDefault from "./d""#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "0".to_string(),
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
        ],
    );
    done();
}

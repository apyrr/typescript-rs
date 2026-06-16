#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_order2() {
    let mut t = TestingT;
    run_test_import_name_code_fix_order2(&mut t);
}

fn run_test_import_name_code_fix_order2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /a.ts
export const _aB: number;
export const _Ab: number;
export const aB: number;
export const Ab: number;
// @Filename: /b.ts
[|import {
    _aB,
    _Ab,
    Ab,
} from "./a";
aB;|]
// @Filename: /c.ts
[|import {
    _aB,
    _Ab,
    Ab,
} from "./a";
aB;|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import {
    _aB,
    _Ab,
    Ab,
    aB,
} from "./a";
aB;"#
                .to_string(),
        ],
        None,
    );
    f.go_to_file(t, "/c.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import {
    _aB,
    _Ab,
    aB,
    Ab,
} from "./a";
aB;"#
                .to_string(),
        ],
        None,
    );
    done();
}

#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_quote_style() {
    let mut t = TestingT;
    run_test_import_name_code_fix_quote_style(&mut t);
}

fn run_test_import_name_code_fix_quote_style(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_quoteStyle") {
        return;
    }
    let content = r"// @Filename: /a.ts
export const foo: number;
// @Filename: /b.ts
[|foo;|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r"import { foo } from './a';

foo;"
                .to_string(),
        ],
        Some(UserPreferences {
            quote_preference: lsutil::QuotePreference::Single,
            ..Default::default()
        }),
    );
    done();
}

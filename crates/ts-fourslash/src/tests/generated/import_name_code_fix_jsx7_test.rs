#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_jsx7() {
    let mut t = TestingT;
    run_test_import_name_code_fix_jsx7(&mut t);
}

fn run_test_import_name_code_fix_jsx7(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @jsx: react
// @module: esnext
// @esModuleInterop: true
// @moduleResolution: bundler
// @Filename: /node_modules/react/index.d.ts
// React was not defined
// @Filename: /a.tsx
<[|Text|]></Text>;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/a.tsx");
    f.verify_code_fix_not_available(t, &[]);
    done();
}

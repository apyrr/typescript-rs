#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_allow_ts_extensions2() {
    let mut t = TestingT;
    run_test_auto_import_allow_ts_extensions2(&mut t);
}

fn run_test_auto_import_allow_ts_extensions2(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportAllowTsExtensions2") {
        return;
    }
    let content = r"// @moduleResolution: bundler
// @allowImportingTsExtensions: true
// @noEmit: true
// @Filename: /node_modules/@types/foo/index.d.ts
export const fromAtTypesFoo: number;
// @Filename: /node_modules/bar/index.d.ts
export const fromBar: number;
// @Filename: /local.ts
export const fromLocal: number;
// @Filename: /Component.tsx
export function Component() { return null; }
// @Filename: /main.ts
/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.baseline_auto_imports_completions(t, &[]);
    done();
}

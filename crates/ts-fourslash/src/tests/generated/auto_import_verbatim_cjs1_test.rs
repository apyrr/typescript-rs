#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_verbatim_cjs1() {
    let mut t = TestingT;
    run_test_auto_import_verbatim_cjs1(&mut t);
}

fn run_test_auto_import_verbatim_cjs1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @module: node18
// @verbatimModuleSyntax: true
// @allowJs: true
// @Filename: /node_modules/@types/node/path.d.ts
declare module 'path' {
    namespace path {
        interface PlatformPath {
            normalize(p: string): string;
            join(...paths: string[]): string;
            resolve(...pathSegments: string[]): string;
            isAbsolute(p: string): boolean;
         }
    }
    const path: path.PlatformPath;
    export = path;
}
// @Filename: /cool-name.js
module.exports = {
  explode: () => {}
}
// @Filename: /a.ts
/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.baseline_auto_imports_completions(t, &[]);
    done();
}

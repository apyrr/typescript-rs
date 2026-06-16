#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_type_only_import() {
    let mut t = TestingT;
    run_test_go_to_definition_type_only_import(&mut t);
}

fn run_test_go_to_definition_type_only_import(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: /a.ts
enum /*1*/SyntaxKind { SourceFile }
export type { SyntaxKind }
// @Filename: /b.ts
 export type { SyntaxKind } from './a';
// @Filename: /c.ts
import type { SyntaxKind } from './b';
let kind: [|/*2*/SyntaxKind|];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["2".to_string()]);
    done();
}

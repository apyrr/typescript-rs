#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_triple_slash_ordering() {
    let mut t = TestingT;
    run_test_import_name_code_fix_triple_slash_ordering(&mut t);
}

fn run_test_import_name_code_fix_triple_slash_ordering(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_tripleSlashOrdering") {
        return;
    }
    let content = r#"// @Filename: /tsconfig.json
{
    "compilerOptions": {
        "skipDefaultLibCheck": false
    }
}
// @Filename: /a.ts
export const x = 0;
// @Filename: /b.ts
// some comment

/// <reference lib="es2017.string" />

const y = x + 1;
// @Filename: /c.ts
// some comment

/// <reference path="jquery-1.8.3.js" />

const y = x + 1;
// @Filename: /d.ts
// some comment

/// <reference types="node" />

const y = x + 1;
// @Filename: /f.ts
// some comment

/// <amd-module name="NamedModule" />

const y = x + 1;
// @Filename: /g.ts
// some comment

/// <amd-dependency path="legacy/moduleA" name="moduleA" />

const y = x + 1;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"// some comment

/// <reference lib="es2017.string" />

import { x } from "./a";

const y = x + 1;"#
            .to_string()],
        None,
    );
    f.go_to_file(t, "/c.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"// some comment

/// <reference path="jquery-1.8.3.js" />

import { x } from "./a";

const y = x + 1;"#
            .to_string()],
        None,
    );
    f.go_to_file(t, "/d.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"// some comment

/// <reference types="node" />

import { x } from "./a";

const y = x + 1;"#
            .to_string()],
        None,
    );
    f.go_to_file(t, "/f.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"// some comment

/// <amd-module name="NamedModule" />

import { x } from "./a";

const y = x + 1;"#
            .to_string()],
        None,
    );
    f.go_to_file(t, "/g.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"// some comment

/// <amd-dependency path="legacy/moduleA" name="moduleA" />

import { x } from "./a";

const y = x + 1;"#
            .to_string()],
        None,
    );
    done();
}

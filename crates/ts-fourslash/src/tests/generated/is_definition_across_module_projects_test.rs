#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_is_definition_across_module_projects() {
    let mut t = TestingT;
    run_test_is_definition_across_module_projects(&mut t);
}

fn run_test_is_definition_across_module_projects(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /home/src/workspaces/project/a/index.ts
import { NS } from "../b";
import { I } from "../c";

declare module "../b" {
    export namespace NS {
        export function /*1*/FA();
    }
}

declare module "../c" {
    export interface /*2*/I {
        /*3*/FA();
    }
}

const ia: I = {
    FA: NS.FA,
    FC() { },
};
// @Filename: /home/src/workspaces/project/a/tsconfig.json
{
    "extends": "../tsconfig.settings.json",
    "references": [
        { "path": "../b" },
        { "path": "../c" },
    ],
    "files": [
        "index.ts",
    ],
}
// @Filename: /home/src/workspaces/project/a2/index.ts
import { NS } from "../b";
import { I } from "../c";

declare module "../b" {
    export namespace NS {
        export function /*4*/FA();
    }
}

declare module "../c" {
    export interface /*5*/I {
        /*6*/FA();
    }
}

const ia: I = {
    FA: NS.FA,
    FC() { },
};
// @Filename: /home/src/workspaces/project/a2/tsconfig.json
{
    "extends": "../tsconfig.settings.json",
    "references": [
        { "path": "../b" },
        { "path": "../c" },
    ],
    "files": [
        "index.ts",
    ],
}
// @Filename: /home/src/workspaces/project/b/index.ts
export namespace NS {
    export function /*7*/FB() {}
}

export interface /*8*/I {
    /*9*/FB();
}

const ib: I = { FB() {} };
// @Filename: /home/src/workspaces/project/b/other.ts
export const Other = 1;
// @Filename: /home/src/workspaces/project/b/tsconfig.json
{
    "extends": "../tsconfig.settings.json",
    "files": [
        "index.ts",
        "other.ts",
    ],
}
// @Filename: /home/src/workspaces/project/c/index.ts
export namespace NS {
    export function /*10*/FC() {}
}

export interface /*11*/I {
    /*12*/FC();
}

const ic: I = { FC() {} };
// @Filename: /home/src/workspaces/project/c/tsconfig.json
{
    "extends": "../tsconfig.settings.json",
    "files": [
        "index.ts",
    ],
}
// @Filename: /home/src/workspaces/project/tsconfig.json
{
    "compilerOptions": {
        "composite": true,
        "lib": ["es5"],
    },
    "references": [
        { "path": "a" },
        { "path": "a2" },
    ],
    "files": []
}
// @Filename: /home/src/workspaces/project/tsconfig.settings.json
{
    "compilerOptions": {
        "composite": true,
        "skipLibCheck": true,
        "declarationMap": true,
        "module": "CommonJS",
        "emitDeclarationOnly": true,
        "lib": ["es5"],
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
            "8".to_string(),
            "9".to_string(),
            "10".to_string(),
            "11".to_string(),
            "12".to_string(),
        ],
    );
    done();
}

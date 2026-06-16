#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_umd_module_as_global_const() {
    let mut t = TestingT;
    run_test_find_all_references_umd_module_as_global_const(&mut t);
}

fn run_test_find_all_references_umd_module_as_global_const(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /node_modules/@types/three/three-core.d.ts
export class Vector3 {
    constructor(x?: number, y?: number, z?: number);
    x: number;
    y: number;
}
// @Filename: /node_modules/@types/three/index.d.ts
export * from "./three-core";
export as namespace /*0*/THREE;
// @Filename: /typings/global.d.ts
import * as _THREE from '/*1*/three';
declare global {
    const /*2*/THREE: typeof _THREE;
}
// @Filename: /src/index.ts
export const a = {};
let v = new /*3*/THREE.Vector2();
// @Filename: /tsconfig.json
{
    "compilerOptions": {
        "esModuleInterop": true,
        "outDir": "./build/js/",
        "noImplicitAny": true,
        "module": "es6",
        "target": "es6",
        "allowJs": true,
        "skipLibCheck": true,
        "lib": ["es2016", "dom"],
        "typeRoots": ["node_modules/@types/"],
        "types": ["three"]
 	},
    "files": ["/src/index.ts", "typings/global.d.ts"]
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "0".to_string(),
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
        ],
    );
    done();
}

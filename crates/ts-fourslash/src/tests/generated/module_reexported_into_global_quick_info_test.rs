#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_module_reexported_into_global_quick_info() {
    let mut t = TestingT;
    run_test_module_reexported_into_global_quick_info(&mut t);
}

fn run_test_module_reexported_into_global_quick_info(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: /node_modules/@types/three/index.d.ts
export class Vector3 {}
export as namespace THREE;
// @Filename: /global.d.ts
import * as _THREE from 'three';

declare global {
  const THREE: typeof _THREE;
}
// @Filename: /index.ts
let v = new /*1*/THREE.Vector3();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "const THREE: typeof import(\"three\")", "");
    done();
}

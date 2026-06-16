#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_declaration_maps_out_of_date_mapping() {
    let mut t = TestingT;
    run_test_declaration_maps_out_of_date_mapping(&mut t);
}

fn run_test_declaration_maps_out_of_date_mapping(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @lib: es5
// @Filename: /home/src/workspaces/project/node_modules/a/dist/index.d.ts
export declare class Foo {
    bar: any;
}
//# sourceMappingURL=index.d.ts.map
// @Filename: /home/src/workspaces/project/node_modules/a/dist/index.d.ts.map
{"version":3,"file":"index.d.ts","sourceRoot":"","sources":["../src/index.ts"],"names":[],"mappings":"AAAA,qBAAa,GAAG;IACZ,GAAG,MAAC;CACP"}
// @Filename: /home/src/workspaces/project/node_modules/a/src/index.ts
export class /*2*/Foo {
}

// @Filename: /home/src/workspaces/project/node_modules/a/package.json
{
    "name": "a",
    "version": "0.0.0",
    "private": true,
    "main": "dist",
    "types": "dist"
}
// @Filename: /home/src/workspaces/project/index.ts
import { Foo/*1*/ } from "a";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_file(t, "/home/src/workspaces/project/index.ts");
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}

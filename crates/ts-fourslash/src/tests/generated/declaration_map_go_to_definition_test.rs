#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_declaration_map_go_to_definition() {
    let mut t = TestingT;
    run_test_declaration_map_go_to_definition(&mut t);
}

fn run_test_declaration_map_go_to_definition(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @lib: es5
// @Filename: index.ts
export class Foo {
    member: string;
    /*2*/methodName(propName: SomeType): void {}
    otherMethod() {
        if (Math.random() > 0.5) {
            return {x: 42};
        }
        return {y: "yes"};
    }
}

export interface SomeType {
    member: number;
}
// @Filename: indexdef.d.ts.map
{"version":3,"file":"indexdef.d.ts","sourceRoot":"","sources":["index.ts"],"names":[],"mappings":"AAAA;IACI,MAAM,EAAE,MAAM,CAAC;IACf,UAAU,CAAC,QAAQ,EAAE,QAAQ,GAAG,IAAI;IACpC,WAAW;;;;;;;CAMd;AAED,MAAM,WAAW,QAAQ;IACrB,MAAM,EAAE,MAAM,CAAC;CAClB"}
// @Filename: indexdef.d.ts
export declare class Foo {
    member: string;
    methodName(propName: SomeType): void;
    otherMethod(): {
        x: number;
        y?: undefined;
    } | {
        y: string;
        x?: undefined;
    };
}
export interface SomeType {
    member: number;
}
//# sourceMappingURL=indexdef.d.ts.map
// @Filename: mymodule.ts
import * as mod from "./indexdef";
const instance = new mod.Foo();
instance.[|/*1*/methodName|]({member: 12});"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}

#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_duplicate_package_services() {
    let mut t = TestingT;
    run_test_duplicate_package_services(&mut t);
}

fn run_test_duplicate_package_services(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @noImplicitReferences: true
// @Filename: /node_modules/a/index.d.ts
import [|X/*useAX*/|] from "x";
export function a(x: X): void;
// @Filename: /node_modules/a/node_modules/x/index.d.ts
export default class /*defAX*/X {
    private x: number;
}
// @Filename: /node_modules/a/node_modules/x/package.json
{ "name": "x", "version": "1.2.3" }
// @Filename: /node_modules/b/index.d.ts
import [|X/*useBX*/|] from "x";
export const b: X;
// @Filename: /node_modules/b/node_modules/x/index.d.ts
export default class /*defBX*/X {
    private x: number;
}
// @Filename: /node_modules/b/node_modules/x/package.json
{ "name": "x", "version": "1.2.3" }
// @Filename: /src/a.ts
import { a } from "a";
import { b } from "b";
a(/*error*/b);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/src/a.ts");
    f.verify_number_of_errors_in_current_file(0);
    f.verify_baseline_find_all_references(
        t,
        &[
            "useAX".to_string(),
            "defAX".to_string(),
            "useBX".to_string(),
        ],
    );
    f.verify_baseline_go_to_definition(t, &["useAX".to_string(), "useBX".to_string()]);
    done();
}

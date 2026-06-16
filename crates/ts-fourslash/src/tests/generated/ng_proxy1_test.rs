#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_ng_proxy1() {
    let mut t = TestingT;
    run_test_ng_proxy1(&mut t);
}

fn run_test_ng_proxy1(t: &mut TestingT) {
    if should_skip_if_failing("TestNgProxy1") {
        return;
    }
    let content = r#"// @Filename: tsconfig.json
{
    "compilerOptions": {
        "lib": ["es5"],
        "plugins": [
            { "name": "quickinfo-augmeneter", "message": "hello world" }
        ]
    },
    "files": ["a.ts"]
}
// @Filename: a.ts
let x = [1, 2];
x/**/
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "");
    f.verify_quick_info_is(t, "Proxied x: number[]hello world", "");
    done();
}

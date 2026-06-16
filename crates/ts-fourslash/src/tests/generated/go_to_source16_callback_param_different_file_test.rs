#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_source16_callback_param_different_file() {
    let mut t = TestingT;
    run_test_go_to_source16_callback_param_different_file(&mut t);
}

fn run_test_go_to_source16_callback_param_different_file(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToSource16_callbackParamDifferentFile") {
        return;
    }
    let content = r#"// @lib: es5
// @moduleResolution: bundler
// @Filename: /home/src/workspaces/project/node_modules/@types/yargs/package.json
{
    "name": "@types/yargs",
    "version": "1.0.0",
    "types": "./index.d.ts"
}
// @Filename: /home/src/workspaces/project/node_modules/@types/yargs/callback.d.ts
export declare class Yargs { positional(): Yargs; }
// @Filename: /home/src/workspaces/project/node_modules/@types/yargs/index.d.ts
import { Yargs } from "./callback";
export declare function command(command: string, cb: (yargs: Yargs) => void): void;
// @Filename: /home/src/workspaces/project/node_modules/yargs/package.json
{
    "name": "yargs",
    "version": "1.0.0",
    "main": "index.js"
}
// @Filename: /home/src/workspaces/project/node_modules/yargs/callback.js
export class Yargs { positional() { } }
// @Filename: /home/src/workspaces/project/node_modules/yargs/index.js
import { Yargs } from "./callback";
export function command(cmd, cb) { cb(Yargs) }
// @Filename: /home/src/workspaces/project/index.ts
import { command } from "yargs";
command("foo", yargs => {
    yargs.[|/*start*/positional|]();
});"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_go_to_source_definition(t, &["start".to_string()]);
    done();
}

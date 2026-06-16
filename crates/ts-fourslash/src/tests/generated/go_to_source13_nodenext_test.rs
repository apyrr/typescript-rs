#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_source13_nodenext() {
    let mut t = TestingT;
    run_test_go_to_source13_nodenext(&mut t);
}

fn run_test_go_to_source13_nodenext(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /home/src/workspaces/project/node_modules/left-pad/package.json
{
  "name": "left-pad",
  "version": "1.3.0",
  "description": "String left pad",
  "main": "index.js",
  "types": "index.d.ts"
}
// @Filename: /home/src/workspaces/project/node_modules/left-pad/index.d.ts
declare function leftPad(str: string|number, len: number, ch?: string|number): string;
declare namespace leftPad { }
export = leftPad;
// @Filename: /home/src/workspaces/project/node_modules/left-pad/index.js
module.exports = leftPad;
function /*end*/leftPad(str, len, ch) {}
// @Filename: /home/src/workspaces/project/tsconfig.json
{
  "compilerOptions": {
      "module": "node16",
      "lib": ["es5"],
      "strict": true,
      "outDir": "./out",

  }
}
// @Filename: /home/src/workspaces/project/index.mts
import leftPad = require("left-pad");
/*start*/leftPad("", 4);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_go_to_source_definition(t, &["start".to_string()]);
    done();
}

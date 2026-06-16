#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_provider9() {
    let mut t = TestingT;
    run_test_auto_import_provider9(&mut t);
}

fn run_test_auto_import_provider9(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @lib: es5
// @module: preserve
// @Filename: /home/src/workspaces/project/index.ts
Lib1/**/
// @Filename: /home/src/workspaces/project/package.json
{
  "dependencies": {
    "lib1": "*",
    "lib2": "*",
    "lib3": "*",
    "lib4": "*",
    "lib5": "*",
    "lib6": "*",
    "lib7": "*",
    "lib8": "*",
    "lib9": "*",
    "lib10": "*",
    "lib11": "*"
  }
}
// @Filename: /home/src/workspaces/project/node_modules/lib1/package.json
{ "name": "lib1", "types": "./index.d.ts" }
// @Filename: /home/src/workspaces/project/node_modules/lib1/index.d.ts
export class Lib1 {}
// @Filename: /home/src/workspaces/project/node_modules/lib2/package.json
{ "name": "lib2", "types": "./index.d.ts" }
// @Filename: /home/src/workspaces/project/node_modules/lib2/index.d.ts
export class Lib2 {}
// @Filename: /home/src/workspaces/project/node_modules/lib3/package.json
{ "name": "lib3", "types": "./index.d.ts" }
// @Filename: /home/src/workspaces/project/node_modules/lib3/index.d.ts
export class Lib3 {}
// @Filename: /home/src/workspaces/project/node_modules/lib4/package.json
{ "name": "lib4", "types": "./index.d.ts" }
// @Filename: /home/src/workspaces/project/node_modules/lib4/index.d.ts
export class Lib4 {}
// @Filename: /home/src/workspaces/project/node_modules/lib5/package.json
{ "name": "lib5", "types": "./index.d.ts" }
// @Filename: /home/src/workspaces/project/node_modules/lib5/index.d.ts
export class Lib5 {}
// @Filename: /home/src/workspaces/project/node_modules/lib6/package.json
{ "name": "lib6", "types": "./index.d.ts" }
// @Filename: /home/src/workspaces/project/node_modules/lib6/index.d.ts
export class Lib6 {}
// @Filename: /home/src/workspaces/project/node_modules/lib7/package.json
{ "name": "lib7", "types": "./index.d.ts" }
// @Filename: /home/src/workspaces/project/node_modules/lib7/index.d.ts
export class Lib7 {}
// @Filename: /home/src/workspaces/project/node_modules/lib8/package.json
{ "name": "lib8", "types": "./index.d.ts" }
// @Filename: /home/src/workspaces/project/node_modules/lib8/index.d.ts
export class Lib8 {}
// @Filename: /home/src/workspaces/project/node_modules/lib9/package.json
{ "name": "lib9", "types": "./index.d.ts" }
// @Filename: /home/src/workspaces/project/node_modules/lib9/index.d.ts
export class Lib9 {}
// @Filename: /home/src/workspaces/project/node_modules/lib10/package.json
{ "name": "lib10", "types": "./index.d.ts" }
// @Filename: /home/src/workspaces/project/node_modules/lib10/index.d.ts
export class Lib10 {}
// @Filename: /home/src/workspaces/project/node_modules/lib11/package.json
{ "name": "lib11", "types": "./index.d.ts" }
// @Filename: /home/src/workspaces/project/node_modules/lib11/index.d.ts
export class Lib11 {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_import_fix_module_specifiers(t, "", &vec![], None);
    f.verify_import_fix_module_specifiers(t, "", &vec![], None);
    f.insert_line(t, "import {} from 'lib2';");
    f.verify_import_fix_module_specifiers(t, "", &vec!["lib1".to_string()], None);
    done();
}

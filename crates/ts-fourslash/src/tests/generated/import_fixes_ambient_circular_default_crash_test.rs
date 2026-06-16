#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_fixes_ambient_circular_default_crash() {
    let mut t = TestingT;
    run_test_import_fixes_ambient_circular_default_crash(&mut t);
}

fn run_test_import_fixes_ambient_circular_default_crash(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /home/src/workspaces/project/tsconfig.json
{
  "compilerOptions": {
    "module": "preserve",
    "lib": ["es5"]
  }
}
// @Filename: /home/src/workspaces/project/types.d.ts
declare module "mymod" {
  import mymod from "mymod";
  export default mymod;
}
// @Filename: /home/src/workspaces/project/index.ts
my/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_import_fix_module_specifiers(t, "", &vec![], None);
    done();
}

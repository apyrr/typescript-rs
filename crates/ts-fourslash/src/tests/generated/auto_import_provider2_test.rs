#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_provider2() {
    let mut t = TestingT;
    run_test_auto_import_provider2(&mut t);
}

fn run_test_auto_import_provider2(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportProvider2") {
        return;
    }
    let content = r#"// @Filename: /home/src/workspaces/project/node_modules/direct-dependency/package.json
{ "name": "direct-dependency", "dependencies": { "indirect-dependency": "*" } }
// @Filename: /home/src/workspaces/project/node_modules/direct-dependency/index.d.ts
import "indirect-dependency";
export declare class DirectDependency {}
// @Filename: /home/src/workspaces/project/node_modules/indirect-dependency/package.json
{ "name": "indirect-dependency" }
// @Filename: /home/src/workspaces/project/node_modules/indirect-dependency/index.d.ts
export declare class IndirectDependency
// @Filename: /home/src/workspaces/project/tsconfig.json
{ "compilerOptions": { "lib": ["es5"] } }
// @Filename: /home/src/workspaces/project/package.json
{ "dependencies": { "direct-dependency": "*" } }
// @Filename: /home/src/workspaces/project/index.ts
IndirectDependency/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "");
    {
        let mut opts = f.get_options();
        opts.format_code_settings.new_line_character = "\\n".to_string();
        f.configure(t, opts);
    }
    f.verify_import_fix_at_position(t, &[], None);
    done();
}

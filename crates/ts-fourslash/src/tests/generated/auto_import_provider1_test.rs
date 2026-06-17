#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_provider1() {
    let mut t = TestingT;
    run_test_auto_import_provider1(&mut t);
}

fn run_test_auto_import_provider1(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportProvider1") {
        return;
    }
    let content = r#"// @Filename: /home/src/workspaces/project/node_modules/@angular/forms/package.json
{ "name": "@angular/forms", "typings": "./forms.d.ts" }
// @Filename: /home/src/workspaces/project/node_modules/@angular/forms/forms.d.ts
export class PatternValidator {}
// @Filename: /home/src/workspaces/project/tsconfig.json
{ "compilerOptions": { "lib": ["es5"] } }
// @Filename: /home/src/workspaces/project/package.json
{ "dependencies": { "@angular/forms": "*" } }
// @Filename: /home/src/workspaces/project/index.ts
PatternValidator/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "");
    {
        let mut opts = f.get_options();
        opts.format_code_settings.new_line_character = "\\n".to_string();
        f.configure(t, opts);
    }
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import { PatternValidator } from "@angular/forms";

PatternValidator"#
            .to_string()],
        None,
    );
    done();
}

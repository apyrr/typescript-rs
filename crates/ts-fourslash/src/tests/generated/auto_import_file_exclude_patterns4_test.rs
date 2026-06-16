#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_file_exclude_patterns4() {
    let mut t = TestingT;
    run_test_auto_import_file_exclude_patterns4(&mut t);
}

fn run_test_auto_import_file_exclude_patterns4(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportFileExcludePatterns4") {
        return;
    }
    let content = r"// @Filename: /src/vs/workbench/test.ts
import { Parts } from './parts';
export class /**/EditorParts implements Parts { }
// @Filename: /src/vs/event/event.ts
export interface Event {
	(): string;
}
// @Filename: /src/vs/workbench/parts.ts
import { Event } from '../event/event';
export interface Parts {
	readonly options: Event;
}
// @Filename: /src/vs/workbench/workbench.ts
import { Event } from '../event/event';
export { Event };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'Parts'".to_string(),
            new_file_content: r"import { Event } from '../event/event';
import { Parts } from './parts';
export class EditorParts implements Parts {
    options: Event;
}"
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: Some(UserPreferences {
                auto_import_file_exclude_patterns: vec![
                    "src/vs/workbench/workbench.ts".to_string(),
                ],
                ..Default::default()
            }),
        },
    );
    done();
}

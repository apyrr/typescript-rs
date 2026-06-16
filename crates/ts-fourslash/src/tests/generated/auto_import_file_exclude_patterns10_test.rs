#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_file_exclude_patterns10() {
    let mut t = TestingT;
    run_test_auto_import_file_exclude_patterns10(&mut t);
}

fn run_test_auto_import_file_exclude_patterns10(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportFileExcludePatterns10") {
        return;
    }
    let content = r"// @Filename: /src/vs/test.ts
import { Parts } from './parts';
export class /**/Extended implements Parts {
}
// @Filename: /src/vs/parts.ts
import { Event } from '../event/event';

export interface Parts {
	readonly options: Event;
}
// @Filename: /src/event/event.ts
export interface Event {
	(): string;
}
// @Filename: /src/thing.ts
import { Event } from './event/event';
export { Event };
// @Filename: /src/a.ts
import './thing'
declare module './thing' {
	interface Event {
		c: string;
	}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'Parts'".to_string(),
            new_file_content: r"import { Event } from '../event/event';
import { Parts } from './parts';
export class Extended implements Parts {
    options: Event;
}"
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: Some(UserPreferences {
                auto_import_file_exclude_patterns: vec!["src/thing.ts".to_string()],
                ..Default::default()
            }),
        },
    );
    done();
}

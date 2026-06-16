#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_remove_semicolons4() {
    let mut t = TestingT;
    run_test_format_remove_semicolons4(&mut t);
}

fn run_test_format_remove_semicolons4(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatRemoveSemicolons4") {
        return;
    }
    let content = r"declare const opt: number | undefined;

const a = 1;
const b = 2;
;[1, 2, 3]

const c = opt ? 1 : 2;
const d = opt ? 1 : 2;
;[1, 2, 3]

const e = opt ?? 1;
const f = opt ?? 1;
;[1, 2, 3]

type a = 1;
type b = 2;
;[1, 2, 3]

type c = typeof opt extends 1 ? 1 : 2;
type d = typeof opt extends 1 ? 1 : 2;
;[1, 2, 3]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings.semicolons = lsutil::SemicolonPreference::Remove;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"declare const opt: number | undefined

const a = 1
const b = 2
;[1, 2, 3]

const c = opt ? 1 : 2
const d = opt ? 1 : 2
;[1, 2, 3]

const e = opt ?? 1
const f = opt ?? 1
;[1, 2, 3]

type a = 1
type b = 2
;[1, 2, 3]

type c = typeof opt extends 1 ? 1 : 2
type d = typeof opt extends 1 ? 1 : 2
;[1, 2, 3]",
    );
    done();
}

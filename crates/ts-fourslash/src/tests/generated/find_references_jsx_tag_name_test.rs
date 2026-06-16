#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_references_jsx_tag_name() {
    let mut t = TestingT;
    run_test_find_references_jsx_tag_name(&mut t);
}

fn run_test_find_references_jsx_tag_name(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: index.tsx
import { /*1*/SubmissionComp } from "./RedditSubmission"
function displaySubreddit(subreddit: string) {
    let components = submissions
        .map((value, index) => <SubmissionComp key={ index } elementPosition= { index } {...value.data} />);
}
// @Filename: RedditSubmission.ts
export const /*2*/SubmissionComp = (submission: SubmissionProps) =>
    <div style={{ fontFamily: "sans-serif" }}></div>;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}

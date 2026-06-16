#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_object_literal2() {
    let mut t = TestingT;
    run_test_completion_list_in_object_literal2(&mut t);
}

fn run_test_completion_list_in_object_literal2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface TelemetryService {
    publicLog(eventName: string, data: any): any;
};
class SearchResult {
    count() { return 5; }
    isEmpty() { return true; }
    fileCount(): string { return ""; }
}
class Foo {
    public telemetryService: TelemetryService;   // If telemetry service is of type 'any' (i.e. uncomment below line), the drop-down list works
    public telemetryService2;
    private test() {
        var onComplete = (searchResult: SearchResult) => {
            var hasResults = !searchResult.isEmpty();  // Drop-down list on searchResult fine here
            // No drop-down list available on searchResult members within object literal below
            this.telemetryService.publicLog('searchResultsShown', { count: searchResult./*1*/count(), fileCount: searchResult.fileCount() });
            this.telemetryService2.publicLog('searchResultsShown', { count: searchResult./*2*/count(), fileCount: searchResult.fileCount() });
        };
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Markers(f.markers()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![
                    CompletionsExpectedItem::Label("count".to_string()),
                    CompletionsExpectedItem::Label("fileCount".to_string()),
                    CompletionsExpectedItem::Label("isEmpty".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}

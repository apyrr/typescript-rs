use crate::{
    new_fourslash, skip_if_failing, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};
use ts_lsproto as lsproto;

pub fn test_completions_with_string_replacement_mode1(t: &mut TestingT) {
    skip_if_failing("TestCompletionsWithStringReplacementMode1");
    let content = r#"interface TFunction {
    (_: 'login.title', __?: {}): string;
    (_: 'login.description', __?: {}): string;
    (_: 'login.sendEmailAgree', __?: {}): string;
    (_: 'login.termsOfUse', __?: {}): string;
    (_: 'login.privacyPolicy', __?: {}): string;
    (_: 'login.sendEmailButton', __?: {}): string;
    (_: 'login.emailInputPlaceholder', __?: {}): string;
    (_: 'login.errorWrongEmailTitle', __?: {}): string;
    (_: 'login.errorWrongEmailDescription', __?: {}): string;
    (_: 'login.errorGeneralEmailTitle', __?: {}): string;
    (_: 'login.errorGeneralEmailDescription', __?: {}): string;
    (_: 'login.loginErrorTitle', __?: {}): string;
    (_: 'login.loginErrorDescription', __?: {}): string;
    (_: 'login.openEmailAppErrorTitle', __?: {}): string;
    (_: 'login.openEmailAppErrorDescription', __?: {}): string;
    (_: 'login.openEmailAppErrorConfirm', __?: {}): string;
}
const f: TFunction = (() => {}) as any;
f('[|login./**/|]')"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let range = f.ranges()[0].ls_range.clone();
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes: Vec::new(),
            excludes: Vec::new(),
            exact: completion_items(
                &[
                "login.description",
                "login.emailInputPlaceholder",
                "login.errorGeneralEmailDescription",
                "login.errorGeneralEmailTitle",
                "login.errorWrongEmailDescription",
                "login.errorWrongEmailTitle",
                "login.loginErrorDescription",
                "login.loginErrorTitle",
                "login.openEmailAppErrorConfirm",
                "login.openEmailAppErrorDescription",
                "login.openEmailAppErrorTitle",
                "login.privacyPolicy",
                "login.sendEmailAgree",
                "login.sendEmailButton",
                "login.termsOfUse",
                "login.title",
                ],
                range,
            ),
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    };
    f.verify_completions(t, MarkerInput::Name("".to_string()), Some(&expected));
    done();
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

fn completion_items(values: &[&str], range: lsproto::Range) -> Vec<CompletionsExpectedItem> {
    values
        .iter()
        .map(|value| CompletionsExpectedItem::Item(completion_item(value, range.clone())))
        .collect()
}

fn completion_item(label: &str, range: lsproto::Range) -> lsproto::CompletionItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = label.to_string();
    item.text_edit = Some(lsproto::TextEditOrInsertReplaceEdit::TextEdit(lsproto::TextEdit {
        new_text: label.to_string(),
        range,
    }));
    item
}


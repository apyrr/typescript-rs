use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, EditRange, ExpectedCompletionEditRange,
    MarkerInput, TestingT,
};
use ts_lsproto as lsproto;

pub fn test_completion_filter_text1(t: &mut TestingT) {
    let content = r#"
class Foo1 {
    #bar: number;
    constructor(bar: number) {
        this.[|b|]/*1*/
    }
}

class Foo5 {
	#bar: number;
	constructor(bar: number) {
		this./*5*/
	}
}

class Foo2 {
    #bar: number;
    constructor(bar: number) {
        this.[|#b|]/*2*/
    }
}

class Foo6 {
    #bar: number;
    constructor(bar: number) {
        this.[|#|]/*6*/
    }
}

class Foo3 {
    #bar: number;
    constructor(bar: number) {
       [|b|]/*3*/
    }
}

class Foo7 {
	#bar: number;
	constructor(bar: number) {
	   /*7*/
	}
}

class Foo4 {
    #bar: number;
    constructor(bar: number) {
       [|#b|]/*4*/
    }
}

class Foo8 {
    #bar: number;
    constructor(bar: number) {
       [|#|]/*8*/
    }
}
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let ranges = f.ranges();

    verify_bar_completion(
        &mut f,
        t,
        MarkerInput::Name("1".to_string()),
        ExpectedCompletionEditRange::EditRange(EditRange {
            insert: ranges[0].clone(),
            replace: ranges[0].clone(),
        }),
        bar_completion_item(Location::MemberAccessWithFilterText),
    );
    verify_bar_completion(
        &mut f,
        t,
        MarkerInput::Name("5".to_string()),
        ExpectedCompletionEditRange::None,
        bar_completion_item(Location::MemberAccessWithFilterText),
    );
    verify_bar_completion(
        &mut f,
        t,
        MarkerInput::Name("2".to_string()),
        ExpectedCompletionEditRange::EditRange(EditRange {
            insert: ranges[1].clone(),
            replace: ranges[1].clone(),
        }),
        bar_completion_item(Location::MemberAccess),
    );
    verify_bar_completion(
        &mut f,
        t,
        MarkerInput::Name("6".to_string()),
        ExpectedCompletionEditRange::EditRange(EditRange {
            insert: ranges[2].clone(),
            replace: ranges[2].clone(),
        }),
        bar_completion_item(Location::MemberAccess),
    );
    verify_bar_completion(
        &mut f,
        t,
        MarkerInput::Name("3".to_string()),
        ExpectedCompletionEditRange::Ignored,
        bar_completion_item(Location::ClassBodyWithFilterTextAndEdit(ranges[3].ls_range)),
    );
    verify_bar_completion(
        &mut f,
        t,
        MarkerInput::Name("7".to_string()),
        ExpectedCompletionEditRange::None,
        bar_completion_item(Location::ClassBodyWithFilterTextAndInsertText),
    );
    verify_bar_completion(
        &mut f,
        t,
        MarkerInput::Name("4".to_string()),
        ExpectedCompletionEditRange::Ignored,
        bar_completion_item(Location::ClassBodyWithEdit(ranges[4].ls_range)),
    );
    verify_bar_completion(
        &mut f,
        t,
        MarkerInput::Name("8".to_string()),
        ExpectedCompletionEditRange::Ignored,
        bar_completion_item(Location::ClassBodyWithEdit(ranges[5].ls_range)),
    );
    done();
}

fn verify_bar_completion(
    f: &mut crate::FourslashTest,
    t: &mut TestingT,
    marker_input: MarkerInput,
    edit_range: ExpectedCompletionEditRange,
    item: lsproto::CompletionItem,
) {
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range,
        }),
        items: Some(CompletionsExpectedItems {
            includes: vec![CompletionsExpectedItem::Item(item)],
            excludes: Vec::new(),
            exact: Vec::new(),
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    };
    f.verify_completions(t, marker_input, Some(&expected));
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

enum Location {
    MemberAccessWithFilterText,
    MemberAccess,
    ClassBodyWithFilterTextAndEdit(lsproto::Range),
    ClassBodyWithFilterTextAndInsertText,
    ClassBodyWithEdit(lsproto::Range),
}

fn bar_completion_item(location: Location) -> lsproto::CompletionItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = "#bar".to_string();
    item.kind = Some(lsproto::CompletionItemKind::Field);
    match location {
        Location::MemberAccessWithFilterText => {
            item.sort_text = Some("11".to_string());
            item.filter_text = Some("bar".to_string());
        }
        Location::MemberAccess => {
            item.sort_text = Some("11".to_string());
        }
        Location::ClassBodyWithFilterTextAndEdit(range) => {
            item.sort_text = Some("14".to_string());
            item.filter_text = Some("bar".to_string());
            item.text_edit = Some(lsproto::TextEditOrInsertReplaceEdit::InsertReplaceEdit(
                lsproto::InsertReplaceEdit {
                    new_text: "this.#bar".to_string(),
                    insert: range,
                    replace: range,
                },
            ));
        }
        Location::ClassBodyWithFilterTextAndInsertText => {
            item.sort_text = Some("14".to_string());
            item.filter_text = Some("bar".to_string());
            item.insert_text = Some("this.#bar".to_string());
        }
        Location::ClassBodyWithEdit(range) => {
            item.sort_text = Some("14".to_string());
            item.text_edit = Some(lsproto::TextEditOrInsertReplaceEdit::InsertReplaceEdit(
                lsproto::InsertReplaceEdit {
                    new_text: "this.#bar".to_string(),
                    insert: range,
                    replace: range,
                },
            ));
        }
    }
    item
}


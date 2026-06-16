#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_after_property_name() {
    let mut t = TestingT;
    run_test_completion_list_after_property_name(&mut t);
}

fn run_test_completion_list_after_property_name(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: a.ts
class Test1 {
	public some /*afterPropertyName*/
}
// @Filename: b.ts
class Test2 {
	public some(/*inMethodParameter*/
}
// @Filename: c.ts
class Test3 {
	public some(a/*atMethodParameter*/
}
// @Filename: d.ts
class Test4 {
	public some(a /*afterMethodParameter*/
}
// @Filename: e.ts
class Test5 {
	public some(a /*afterMethodParameterBeforeComma*/,
}
// @Filename: f.ts
class Test6 {
	public some(a, /*afterMethodParameterComma*/
}
// @Filename: g.ts
class Test7 {
	constructor(/*inConstructorParameter*/
}
// @Filename: h.ts
class Test8 {
	constructor(public /*inConstructorParameterAfterModifier*/
}
// @Filename: i.ts
class Test9 {
	constructor(a/*atConstructorParameter*/
}
// @Filename: j.ts
class Test10 {
	constructor(public/*atConstructorParameterModifier*/
}
// @Filename: k.ts
class Test11 {
	constructor(public a/*atConstructorParameterAfterModifier*/
}
// @Filename: l.ts
class Test12 {
	constructor(a /*afterConstructorParameter*/
}
// @Filename: m.ts
class Test13 {
	constructor(a /*afterConstructorParameterBeforeComma*/,
}
// @Filename: n.ts
class Test14 {
	constructor(public a, /*afterConstructorParameterComma*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "afterPropertyName".to_string(),
            "inMethodParameter".to_string(),
            "atMethodParameter".to_string(),
            "afterMethodParameter".to_string(),
            "afterMethodParameterBeforeComma".to_string(),
            "afterMethodParameterComma".to_string(),
            "afterConstructorParameter".to_string(),
        ]),
        None,
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "inConstructorParameter".to_string(),
            "inConstructorParameterAfterModifier".to_string(),
            "atConstructorParameter".to_string(),
            "atConstructorParameterModifier".to_string(),
            "atConstructorParameterAfterModifier".to_string(),
            "afterConstructorParameterComma".to_string(),
        ]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_constructor_parameter_keywords(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}

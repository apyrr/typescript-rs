#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_type_alias_intersection_completions() {
    let mut t = TestingT;
    run_test_generic_type_alias_intersection_completions(&mut t);
}

fn run_test_generic_type_alias_intersection_completions(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericTypeAliasIntersectionCompletions") {
        return;
    }
    let content = r"type MixinCtor<A, B> = new () => A & B & { constructor: MixinCtor<A, B> };
function merge<A, B>(a: { prototype: A }, b: { prototype: B }): MixinCtor<A, B> {
  let merged = function() { }
  Object.assign(merged.prototype, a.prototype, b.prototype);
  return <MixinCtor<A, B>><any>merged;
}

class TreeNode {
  value: any;
}

abstract class LeftSideNode extends TreeNode {
  abstract right(): TreeNode;
  left(): TreeNode {
    return null;
  }
}

abstract class RightSideNode extends TreeNode {
  abstract left(): TreeNode;
  right(): TreeNode {
    return null;
  };
}

var obj = new (merge(LeftSideNode, RightSideNode))();
obj./**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: vec![
                    CompletionsExpectedItem::Label("right".to_string()),
                    CompletionsExpectedItem::Label("left".to_string()),
                    CompletionsExpectedItem::Label("value".to_string()),
                    CompletionsExpectedItem::Label("constructor".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    done();
}

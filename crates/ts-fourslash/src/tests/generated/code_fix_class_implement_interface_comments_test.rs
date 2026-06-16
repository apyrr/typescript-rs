#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_comments() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_comments(&mut t);
}

fn run_test_code_fix_class_implement_interface_comments(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es2017
namespace N {
    /**enum prefix */
    export enum /**enum identifier prefix */ E /**open-brace prefix*/ {
    /* literal prefix */ a /** comma prefix */,
    /* literal prefix */ b /** comma prefix */,
    /* literal prefix */ c
    /** close brace prefix */ }
    /** interface prefix */
    export interface /**interface name prefix */ I /**open-brace prefix*/ {
    /** property prefix */ a /** colon prefix */: /** enum literal prefix 1*/ E /** dot prefix */. /** enum literal prefix 2*/a;
    /** property prefix */ b /** colon prefix */: /** enum prefix */ E;
    /**method signature prefix */foo /**open angle prefix */< /**type parameter name prefix */ X /** closing angle prefix */> /**open paren prefix */(/** parameter prefix */ a/** colon prefix */: /** parameter type prefix */ X /** close paren prefix */) /** colon prefix */: /** return type prefix */ string /** semicolon prefix */;
        /**close-brace prefix*/ }
/**close-brace prefix*/ }
class C implements N.I {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(t, VerifyCodeFixOptions {
    description: "Implement interface 'N.I'".to_string(),
    new_file_content: r#"namespace N {
    /**enum prefix */
    export enum /**enum identifier prefix */ E /**open-brace prefix*/ {
    /* literal prefix */ a /** comma prefix */,
    /* literal prefix */ b /** comma prefix */,
    /* literal prefix */ c
    /** close brace prefix */ }
    /** interface prefix */
    export interface /**interface name prefix */ I /**open-brace prefix*/ {
    /** property prefix */ a /** colon prefix */: /** enum literal prefix 1*/ E /** dot prefix */. /** enum literal prefix 2*/a;
    /** property prefix */ b /** colon prefix */: /** enum prefix */ E;
    /**method signature prefix */foo /**open angle prefix */< /**type parameter name prefix */ X /** closing angle prefix */> /**open paren prefix */(/** parameter prefix */ a/** colon prefix */: /** parameter type prefix */ X /** close paren prefix */) /** colon prefix */: /** return type prefix */ string /** semicolon prefix */;
        /**close-brace prefix*/ }
/**close-brace prefix*/ }
class C implements N.I {
    a: N.E.a;
    b: N.E;
    foo<X /** closing angle prefix */>(a: X /** close paren prefix */): string /** semicolon prefix */ {
        throw new Error("Method not implemented.");
    }
}"#.to_string(),
    new_range_content: String::new(),
    index: 0,
    apply_changes: false,
    user_preferences: None,
});
    done();
}

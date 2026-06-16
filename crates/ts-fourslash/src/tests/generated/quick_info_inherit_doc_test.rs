#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_inherit_doc() {
    let mut t = TestingT;
    run_test_quick_info_inherit_doc(&mut t);
}

fn run_test_quick_info_inherit_doc(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoInheritDoc") {
        return;
    }
    let content = r"// @noEmit: true
// @allowJs: true
// @Filename: quickInfoInheritDoc.ts
abstract class BaseClass {
    /**
     * Useful description always applicable
     * 
     * @returns {string} Useful description of return value always applicable.
     */
    public static doSomethingUseful(stuff?: any): string {
        throw new Error('Must be implemented by subclass');
    }

    /**
     * BaseClass.func1
     * @param {any} stuff1 BaseClass.func1.stuff1
     * @returns {void} BaseClass.func1.returns
     */
    public static func1(stuff1: any): void {
    }

    /**
     * Applicable description always.
     */
    public static readonly someProperty: string = 'general value';
}




class SubClass extends BaseClass {

    /**
     * @inheritDoc
     * 
     * @param {{ tiger: string; lion: string; }} [mySpecificStuff] Description of my specific parameter.
     */
    public static /*1*/doSomethingUseful(mySpecificStuff?: { tiger: string; lion: string; }): string {
        let useful = '';

        // do something useful to useful

        return useful;
    }

    /**
     * @inheritDoc
     * @param {any} stuff1 SubClass.func1.stuff1
     * @returns {void} SubClass.func1.returns
     */
    public static /*2*/func1(stuff1: any): void {
    }

    /**
     * text over tag
     * @inheritDoc
     * text after tag
     */
    public static readonly /*3*/someProperty: string = 'specific to this class value'
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}

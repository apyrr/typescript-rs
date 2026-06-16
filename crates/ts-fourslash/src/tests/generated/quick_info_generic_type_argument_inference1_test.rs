#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_generic_type_argument_inference1() {
    let mut t = TestingT;
    run_test_quick_info_generic_type_argument_inference1(&mut t);
}

fn run_test_quick_info_generic_type_argument_inference1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
namespace Underscore {
    export interface Iterator<T, U> {
        (value: T, index: any, list: any): U;
    }

    export interface Static {
        all<T>(list: T[], iterator?: Iterator<T, boolean>, context?: any): T;
        identity<T>(value: T): T;
    }
}

declare var _: Underscore.Static;
var /*1*/r = _./*11*/all([true, 1, null, 'yes'], x => !x);
var /*2*/r2 = _./*21*/all([true], _.identity);
var /*3*/r3 = _./*31*/all([], _.identity);
var /*4*/r4 = _./*41*/all([<any>true], _.identity);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var r: string | number | boolean", "");
    f.verify_quick_info_at(t, "11", "(method) Underscore.Static.all<string | number | boolean>(list: (string | number | boolean)[], iterator?: Underscore.Iterator<string | number | boolean, boolean>, context?: any): string | number | boolean", "");
    f.verify_quick_info_at(t, "2", "var r2: boolean", "");
    f.verify_quick_info_at(t, "21", "(method) Underscore.Static.all<boolean>(list: boolean[], iterator?: Underscore.Iterator<boolean, boolean>, context?: any): boolean", "");
    f.verify_quick_info_at(t, "3", "var r3: any", "");
    f.verify_quick_info_at(t, "31", "(method) Underscore.Static.all<any>(list: any[], iterator?: Underscore.Iterator<any, boolean>, context?: any): any", "");
    f.verify_quick_info_at(t, "4", "var r4: any", "");
    f.verify_quick_info_at(t, "41", "(method) Underscore.Static.all<any>(list: any[], iterator?: Underscore.Iterator<any, boolean>, context?: any): any", "");
    f.verify_no_errors();
    done();
}

#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_object_literal_call_signatures() {
    let mut t = TestingT;
    run_test_object_literal_call_signatures(&mut t);
}

fn run_test_object_literal_call_signatures(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
var /*1*/x: {
    func1(x: number): number;         // Method signature
    func2: (x: number) => number;     // Function type literal
    func3: { (x: number): number };   // Object type literal
};

x.func1 = x.func2 = x.func3;

var /*2*/y: {
    func4(x: number): number;
    func4(s: string): string;
    func5: {
        (x: number): number;
        (s: string): string;
    };
};

y.func4 = y.func5;
y.func5 = y.func4;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_error_exists_after_marker_name("1");
    f.verify_quick_info_at(t, "1", "var x: {\n    func1(x: number): number;\n    func2: (x: number) => number;\n    func3: {\n        (x: number): number;\n    };\n}", "");
    f.verify_quick_info_at(t, "2", "var y: {\n    func4(x: number): number;\n    func4(s: string): string;\n    func5: {\n        (x: number): number;\n        (s: string): string;\n    };\n}", "");
    done();
}

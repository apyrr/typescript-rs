#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navbar01() {
    let mut t = TestingT;
    run_test_navbar01(&mut t);
}

fn run_test_navbar01(t: &mut TestingT) {
    if should_skip_if_failing("TestNavbar01") {
        return;
    }
    let content = r"// @lib: es5
// Interface
interface IPoint {
    getDist(): number;
    new(): IPoint;
    (): any;
    [x:string]: number;
    prop: string;
}

/// Module
namespace Shapes {
    // Class
    export class Point implements IPoint {
        constructor (public x: number, public y: number) { }

        // Instance member
        getDist() { return Math.sqrt(this.x * this.x + this.y * this.y); }

        // Getter
        get value(): number { return 0; }

        // Setter
        set value(newValue: number) { return; }

        // Static member
        static origin = new Point(0, 0);

        // Static method
        private static getOrigin() { return Point.origin;}
    }

    enum Values { value1, value2, value3 }
}

// Local variables
var p: IPoint = new Shapes.Point(3, 4);
var dist = p.getDist();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_document_symbol(t);
    done();
}

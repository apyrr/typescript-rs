#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_spelling_js6() {
    let mut t = TestingT;
    run_test_code_fix_spelling_js6(&mut t);
}

fn run_test_code_fix_spelling_js6(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowjs: true
// @checkjs: false
// @noEmit: true
// @filename: spellingUncheckedJS.js
export var inModule = 1
inmodule.toFixed()

function f() {
    var locals = 2 + true
    locale.toFixed()
}
class Classe {
    non = 'oui'
    methode() {
        // no error on 'this' references
        return this.none
    }
}
class Derivee extends Classe {
    methode() {
        // no error on 'super' references
        return super.none
    }
}


var object = {
    spaaace: 3
}
object.spaaaace // error on read
object.spaace = 12 // error on write
object.fresh = 12 // OK
other.puuuce // OK, from another file
new Date().getGMTDate() // OK, from another file

// No suggestions for globals from other files
const atoc = setIntegral(() => console.log('ok'), 500)
AudioBuffin // etc
Jimmy
Jon
window.argle
self.blargle
// @filename: other.js
var Jimmy = 1
var John = 2
Jon // error, it's from the same file
var other = {
    puuce: 4
}
window.argle
self.blargle";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    done();
}

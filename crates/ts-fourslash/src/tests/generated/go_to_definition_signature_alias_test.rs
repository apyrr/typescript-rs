#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_signature_alias() {
    let mut t = TestingT;
    run_test_go_to_definition_signature_alias(&mut t);
}

fn run_test_go_to_definition_signature_alias(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionSignatureAlias") {
        return;
    }
    let content = r"// @jsx: preserve
// @Filename: /a.tsx
function /*f*/f() {}
const /*g*/g = f;
const /*h*/h = g;
[|/*useF*/f|]();
[|/*useG*/g|]();
[|/*useH*/h|]();
const /*i*/i = () => 0;
const /*iFn*/iFn = function () { return 0; };
const /*j*/j = i;
[|/*useI*/i|]();
[|/*useIFn*/iFn|]();
[|/*useJ*/j|]();
const o = { /*m*/m: () => 0 };
o.[|/*useM*/m|]();
const oFn = { /*mFn*/mFn: function () { return 0; } };
oFn.[|/*useMFn*/mFn|]();
class Component { /*componentCtr*/constructor(props: {}) {} }
type ComponentClass = /*ComponentClass*/new () => Component;
interface ComponentClass2 { /*ComponentClass2*/new(): Component; }

class /*MyComponent*/MyComponent extends Component {}
<[|/*jsxMyComponent*/MyComponent|] />;
new [|/*newMyComponent*/MyComponent|]({});

declare const /*MyComponent2*/MyComponent2: ComponentClass;
<[|/*jsxMyComponent2*/MyComponent2|] />;
new [|/*newMyComponent2*/MyComponent2|]();

declare const /*MyComponent3*/MyComponent3: ComponentClass2;
<[|/*jsxMyComponent3*/MyComponent3|] />;
new [|/*newMyComponent3*/MyComponent3|]();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_go_to_definition(
        t,
        &[
            "useF".to_string(),
            "useG".to_string(),
            "useH".to_string(),
            "useI".to_string(),
            "useIFn".to_string(),
            "useJ".to_string(),
            "useM".to_string(),
            "useMFn".to_string(),
            "jsxMyComponent".to_string(),
            "newMyComponent".to_string(),
            "jsxMyComponent2".to_string(),
            "newMyComponent2".to_string(),
            "jsxMyComponent3".to_string(),
            "newMyComponent3".to_string(),
        ],
    );
    done();
}

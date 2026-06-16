#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_jsx_texts2() {
    let mut t = TestingT;
    run_test_formatting_jsx_texts2(&mut t);
}

fn run_test_formatting_jsx_texts2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"//@Filename: file.tsx
const a = (
    <div>
  foo
          </div>
);

const b = (
    <div>
  {     foo  }
          </div>
);

const c = (
    <div>
    foo
  {     foobar  }
  bar
          </div>
);

const d = 
    <div>
  foo
          </div>;

const e = 
    <div>
  {     foo  }
          </div>

const f = 
    <div>
    foo
  {     foobar  }
  bar
          </div>";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"const a = (
    <div>
        foo
    </div>
);

const b = (
    <div>
        {foo}
    </div>
);

const c = (
    <div>
        foo
        {foobar}
        bar
    </div>
);

const d =
    <div>
        foo
    </div>;

const e =
    <div>
        {foo}
    </div>

const f =
    <div>
        foo
        {foobar}
        bar
    </div>",
    );
    done();
}

#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_multiline_types_with_mapped() {
    let mut t = TestingT;
    run_test_format_multiline_types_with_mapped(&mut t);
}

fn run_test_format_multiline_types_with_mapped(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatMultilineTypesWithMapped") {
        return;
    }
    let content = r"type Z = 'z'
type A = {
  a: 'a'
} | {
      [index in Z]: string
  }
type B = {
  b: 'b'
} & {
      [index in Z]: string
  }

const c = {
  c: 'c'
} as const satisfies {
    [index in Z]: string
  }

const d = {
  d: 'd'
} as const satisfies {
  [index: string]: string
}

const e = {
  e: 'e'
} satisfies {
    [index in Z]: string
  }

const f = {
  f: 'f'
} satisfies {
  [index: string]: string
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"type Z = 'z'
type A = {
    a: 'a'
} | {
    [index in Z]: string
}
type B = {
    b: 'b'
} & {
    [index in Z]: string
}

const c = {
    c: 'c'
} as const satisfies {
    [index in Z]: string
}

const d = {
    d: 'd'
} as const satisfies {
    [index: string]: string
}

const e = {
    e: 'e'
} satisfies {
    [index in Z]: string
}

const f = {
    f: 'f'
} satisfies {
    [index: string]: string
}",
    );
    done();
}

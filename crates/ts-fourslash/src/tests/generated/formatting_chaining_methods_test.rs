#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_chaining_methods() {
    let mut t = TestingT;
    run_test_formatting_chaining_methods(&mut t);
}

fn run_test_formatting_chaining_methods(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r" z$ = this.store.select(this.fake())
     .ofType(
      'ACTION',
      'ACTION-2'
     )
     .pipe(
         filter(x => !!x),
         switchMap(() =>
          this.store.select(this.menuSelector.getAll('x'))
           .pipe(
             tap(x => {
             this.x = !x;
             })
           )
         )
     );

1
    .toFixed(
        2);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"z$ = this.store.select(this.fake())
    .ofType(
        'ACTION',
        'ACTION-2'
    )
    .pipe(
        filter(x => !!x),
        switchMap(() =>
            this.store.select(this.menuSelector.getAll('x'))
                .pipe(
                    tap(x => {
                        this.x = !x;
                    })
                )
        )
    );

1
    .toFixed(
        2);",
    );
    done();
}

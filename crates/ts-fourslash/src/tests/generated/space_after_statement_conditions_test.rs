#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_space_after_statement_conditions() {
    let mut t = TestingT;
    run_test_space_after_statement_conditions(&mut t);
}

fn run_test_space_after_statement_conditions(t: &mut TestingT) {
    if should_skip_if_failing("TestSpaceAfterStatementConditions") {
        return;
    }
    let content = r"let i = 0;

if(i<0) ++i;
if(i<0) --i;

while(i<0) ++i;
while(i<0) --i;

do ++i;
while(i<0)
do --i;
while(i<0)

for(let prop in { foo: 1 }) ++i;
for(let prop in { foo: 1 }) --i;

for(let foo of [1, 2]) ++i;
for(let foo of [1, 2]) --i;

for(let j = 0; j < 10; j++) ++i;
for(let j = 0; j < 10; j++) --i;
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"let i = 0;

if (i < 0) ++i;
if (i < 0) --i;

while (i < 0) ++i;
while (i < 0) --i;

do ++i;
while (i < 0)
do --i;
while (i < 0)

for (let prop in { foo: 1 }) ++i;
for (let prop in { foo: 1 }) --i;

for (let foo of [1, 2]) ++i;
for (let foo of [1, 2]) --i;

for (let j = 0; j < 10; j++) ++i;
for (let j = 0; j < 10; j++) --i;
",
    );
    done();
}

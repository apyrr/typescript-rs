#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_jsx_texts1() {
    let mut t = TestingT;
    run_test_formatting_jsx_texts1(&mut t);
}

fn run_test_formatting_jsx_texts1(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingJsxTexts1") {
        return;
    }
    let content = r"//@Filename: file.tsx
<option>
    homu   ;      homu
    homu;homu
    homu   :    homu
    homu:homu
    homu    ?     homu
    homu    .    homu

    homu    [   homu   ]   homu

    !     homu
    --    Type
    homu    --
    homu    ++
    ++     homu

    homu  ,   homu

    var    homu
    throw    homu
    new    homu
    delete   homu
    return       homu
    typeof     homu
    await     homu

    abstract  homu
    class     homu
    declare   homu
    default   homu
    enum      homu
    export    homu
    homu    extends   homu
    get       homu
    homu    implements     homu
    interface      homu
    module    homu
    namespace      homu
    private   homu
    public    homu
    protected      homu
    set       homu
    static    homu
    type      homu

    homu    =>    homu
    homu=>homu

    ...       homu

    homu     @     homu
    homu@homu

    (    homu   )    homu
</option>;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"<option>
    homu   ;      homu
    homu;homu
    homu   :    homu
    homu:homu
    homu    ?     homu
    homu    .    homu

    homu    [   homu   ]   homu

    !     homu
    --    Type
    homu    --
    homu    ++
    ++     homu

    homu  ,   homu

    var    homu
    throw    homu
    new    homu
    delete   homu
    return       homu
    typeof     homu
    await     homu

    abstract  homu
    class     homu
    declare   homu
    default   homu
    enum      homu
    export    homu
    homu    extends   homu
    get       homu
    homu    implements     homu
    interface      homu
    module    homu
    namespace      homu
    private   homu
    public    homu
    protected      homu
    set       homu
    static    homu
    type      homu

    homu    =>    homu
    homu=>homu

    ...       homu

    homu     @     homu
    homu@homu

    (    homu   )    homu
</option>;",
    );
    done();
}

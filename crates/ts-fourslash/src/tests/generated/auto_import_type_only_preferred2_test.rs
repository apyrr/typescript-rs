#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_type_only_preferred2() {
    let mut t = TestingT;
    run_test_auto_import_type_only_preferred2(&mut t);
}

fn run_test_auto_import_type_only_preferred2(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportTypeOnlyPreferred2") {
        return;
    }
    let content = r#"// @Filename: /node_modules/react/index.d.ts
export interface ComponentType {}
export interface ComponentProps {}
export declare function useState<T>(initialState: T): [T, (newState: T) => void];
export declare function useEffect(callback: () => void, deps: any[]): void;
// @Filename: /main.ts
import type { ComponentType } from "react";
import { useState } from "react";

export function Component({ prop } : { prop: ComponentType }) {
    const codeIsUnimportant = useState(1);
    useEffect/*1*/(() => {}, []);
}
// @Filename: /main2.ts
import { useState } from "react";
import type { ComponentType } from "react";

type _ = ComponentProps/*2*/;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import type { ComponentType } from "react";
import { useEffect, useState } from "react";

export function Component({ prop } : { prop: ComponentType }) {
    const codeIsUnimportant = useState(1);
    useEffect(() => {}, []);
}"#
            .to_string(),
        ],
        None,
    );
    f.go_to_marker(t, "2");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { useState } from "react";
import type { ComponentProps, ComponentType } from "react";

type _ = ComponentProps;"#
                .to_string(),
        ],
        None,
    );
    done();
}

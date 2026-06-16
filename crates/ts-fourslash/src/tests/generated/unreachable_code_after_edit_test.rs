#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unreachable_code_after_edit() {
    let mut t = TestingT;
    run_test_unreachable_code_after_edit(&mut t);
}

fn run_test_unreachable_code_after_edit(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowUnreachableCode: false
// @lib: es2015
// @Filename: /base/browser/browser.ts
export const isStandalone = true;
// @Filename: /base/browser/dom.ts
export function addDisposableListener() {}
// @Filename: /base/browser/window.ts
export const mainWindow = {} as Window;
// @Filename: /workbench.ts
/*before*/import { isStandalone } from './base/browser/browser';
import { addDisposableListener } from './base/browser/dom';
import { mainWindow } from './base/browser/window';

interface ISecretStorageCrypto {
    seal(data: string): Promise<string>;
    unseal(data: string): Promise<string>;
}

export class TransparentCrypto implements ISecretStorageCrypto {
    async seal(data: string): Promise<string> {
        return data;
    }
    async unseal(data: string): Promise<string> {
        return data;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_number_of_errors_in_current_file(0);
    f.go_to_marker(t, "before");
    f.insert(t, "throw new Error('foo');\n");
    f.verify_number_of_errors_in_current_file(1);
    f.go_to_marker(t, "before");
    f.delete_at_caret(t, 24);
    f.verify_number_of_errors_in_current_file(0);
    done();
}

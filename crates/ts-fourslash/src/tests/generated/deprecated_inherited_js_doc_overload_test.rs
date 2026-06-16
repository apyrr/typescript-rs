#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_deprecated_inherited_js_doc_overload() {
    let mut t = TestingT;
    run_test_deprecated_inherited_js_doc_overload(&mut t);
}

fn run_test_deprecated_inherited_js_doc_overload(t: &mut TestingT) {
    if should_skip_if_failing("TestDeprecatedInheritedJSDocOverload") {
        return;
    }
    let content = r"// @strict: false
interface PartialObserver<T> {}
interface Subscription {}
interface Unsubscribable {}

export interface Subscribable<T> {
  subscribe(observer?: PartialObserver<T>): Unsubscribable;
  /** @deprecated Base deprecation 1 */
  subscribe(next: null | undefined, error: null | undefined, complete: () => void): Unsubscribable;
  /** @deprecated Base deprecation 2 */
  subscribe(next: null | undefined, error: (error: any) => void, complete?: () => void): Unsubscribable;
  /** @deprecated Base deprecation 3 */
  subscribe(next: (value: T) => void, error: null | undefined, complete: () => void): Unsubscribable;
  subscribe(next?: (value: T) => void, error?: (error: any) => void, complete?: () => void): Unsubscribable;
}
interface ThingWithDeprecations<T> extends Subscribable<T> {
   subscribe(observer?: PartialObserver<T>): Subscription;
   /** @deprecated 'real' deprecation */
   subscribe(next: null | undefined, error: null | undefined, complete: () => void): Subscription;
   /** @deprecated 'real' deprecation */
   subscribe(next: null | undefined, error: (error: any) => void, complete?: () => void): Subscription;
}
declare const a: ThingWithDeprecations<void>
a.subscribe/**/(() => {
  console.log('something happened');
});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}

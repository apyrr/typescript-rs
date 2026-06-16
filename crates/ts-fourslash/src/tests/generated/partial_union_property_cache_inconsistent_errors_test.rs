#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_partial_union_property_cache_inconsistent_errors() {
    let mut t = TestingT;
    run_test_partial_union_property_cache_inconsistent_errors(&mut t);
}

fn run_test_partial_union_property_cache_inconsistent_errors(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @strict: true
// @lib: esnext
interface ComponentOptions<Props> {
  setup?: (props: Props) => void;
  name?: string;
}

interface FunctionalComponent<P> {
  (props: P): void;
}

type ConcreteComponent<Props> =
  | ComponentOptions<Props>
  | FunctionalComponent<Props>;

type Component<Props = {}> = ConcreteComponent<Props>;

type WithInstallPlugin = { _prefix?: string };


/**/
export function withInstall<C extends Component, T extends WithInstallPlugin>(
  component: C | C[],
  target?: T,
): string {
  const componentWithInstall = (target ?? component) as T;
  const components = Array.isArray(component) ? component : [component];

  const { name } = components[0];
  if (name) {
    return name;
  }

  return "";
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.go_to_marker(t, "");
    f.insert(t, "type C = Component['name']");
    f.verify_no_errors();
    done();
}

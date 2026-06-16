#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_source7_conditionally_minified() {
    let mut t = TestingT;
    run_test_go_to_source7_conditionally_minified(&mut t);
}

fn run_test_go_to_source7_conditionally_minified(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @lib: es5
// @moduleResolution: bundler
// @Filename: /home/src/workspaces/project/node_modules/react/package.json
{ "name": "react", "version": "16.8.6", "main": "index.js" }
// @Filename: /home/src/workspaces/project/node_modules/react/index.js
'use strict';

if (process.env.NODE_ENV === 'production') {
  module.exports = require('./cjs/react.production.min.js');
} else {
  module.exports = require('./cjs/react.development.js');
}
// @Filename: /home/src/workspaces/project/node_modules/react/cjs/react.production.min.js
'use strict';exports./*production*/useState=function(a){};exports.version='16.8.6';
// @Filename: /home/src/workspaces/project/node_modules/react/cjs/react.development.js
'use strict';
if (process.env.NODE_ENV !== 'production') {
  (function() {
    function useState(initialState) {}
    exports./*development*/useState = useState;
    exports.version = '16.8.6';
  }());
}
// @Filename: /home/src/workspaces/project/index.ts
import { [|/*start*/useState|] } from 'react';"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_go_to_source_definition(t, &["start".to_string()]);
    done();
}

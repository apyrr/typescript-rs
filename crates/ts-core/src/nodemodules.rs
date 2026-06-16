use std::{collections::HashSet, sync::OnceLock};

// require('module').builtinModules.filter(x => !x.match(/^(?:_|node:)/))
pub static UNPREFIXED_NODE_CORE_MODULES: &[&str] = &[
    "assert",
    "assert/strict",
    "async_hooks",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "diagnostics_channel",
    "dns",
    "dns/promises",
    "domain",
    "events",
    "fs",
    "fs/promises",
    "http",
    "http2",
    "https",
    "inspector",
    "inspector/promises",
    "module",
    "net",
    "os",
    "path",
    "path/posix",
    "path/win32",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "readline/promises",
    "repl",
    "stream",
    "stream/consumers",
    "stream/promises",
    "stream/web",
    "string_decoder",
    "sys",
    "timers",
    "timers/promises",
    "tls",
    "trace_events",
    "tty",
    "url",
    "util",
    "util/types",
    "v8",
    "vm",
    "wasi",
    "worker_threads",
    "zlib",
];

// require('module').builtinModules.filter(x => x.startsWith('node:'))
pub static EXCLUSIVELY_PREFIXED_NODE_CORE_MODULES: &[&str] = &[
    "node:quic",
    "node:sea",
    "node:sqlite",
    "node:test",
    "node:test/reporters",
];

static NODE_CORE_MODULES: OnceLock<HashSet<String>> = OnceLock::new();

pub fn node_core_modules() -> &'static HashSet<String> {
    NODE_CORE_MODULES.get_or_init(|| {
        let mut node_core_modules = HashSet::with_capacity(
            UNPREFIXED_NODE_CORE_MODULES.len() * 2 + EXCLUSIVELY_PREFIXED_NODE_CORE_MODULES.len(),
        );
        for unprefixed in UNPREFIXED_NODE_CORE_MODULES {
            node_core_modules.insert((*unprefixed).to_string());
            node_core_modules.insert(format!("node:{unprefixed}"));
        }
        for prefixed in EXCLUSIVELY_PREFIXED_NODE_CORE_MODULES {
            node_core_modules.insert((*prefixed).to_string());
        }
        node_core_modules
    })
}

pub fn non_relative_module_name_for_typing_cache(module_name: &str) -> String {
    if node_core_modules().contains(module_name) {
        return "node".to_string();
    }
    module_name.to_string()
}

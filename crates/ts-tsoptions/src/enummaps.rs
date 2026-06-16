use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::CompilerOptionsValue;

pub type EnumMap = BTreeMap<String, CompilerOptionsValue>;

const LIB_ENTRIES: &[(&str, &str)] = &[
    ("es5", "lib.es5.d.ts"),
    ("es6", "lib.es2015.d.ts"),
    ("es2015", "lib.es2015.d.ts"),
    ("es7", "lib.es2016.d.ts"),
    ("es2016", "lib.es2016.d.ts"),
    ("es2017", "lib.es2017.d.ts"),
    ("es2018", "lib.es2018.d.ts"),
    ("es2019", "lib.es2019.d.ts"),
    ("es2020", "lib.es2020.d.ts"),
    ("es2021", "lib.es2021.d.ts"),
    ("es2022", "lib.es2022.d.ts"),
    ("es2023", "lib.es2023.d.ts"),
    ("es2024", "lib.es2024.d.ts"),
    ("es2025", "lib.es2025.d.ts"),
    ("esnext", "lib.esnext.d.ts"),
    ("dom", "lib.dom.d.ts"),
    ("dom.iterable", "lib.dom.iterable.d.ts"),
    ("dom.asynciterable", "lib.dom.asynciterable.d.ts"),
    ("webworker", "lib.webworker.d.ts"),
    (
        "webworker.importscripts",
        "lib.webworker.importscripts.d.ts",
    ),
    ("webworker.iterable", "lib.webworker.iterable.d.ts"),
    (
        "webworker.asynciterable",
        "lib.webworker.asynciterable.d.ts",
    ),
    ("scripthost", "lib.scripthost.d.ts"),
    ("es2015.core", "lib.es2015.core.d.ts"),
    ("es2015.collection", "lib.es2015.collection.d.ts"),
    ("es2015.generator", "lib.es2015.generator.d.ts"),
    ("es2015.iterable", "lib.es2015.iterable.d.ts"),
    ("es2015.promise", "lib.es2015.promise.d.ts"),
    ("es2015.proxy", "lib.es2015.proxy.d.ts"),
    ("es2015.reflect", "lib.es2015.reflect.d.ts"),
    ("es2015.symbol", "lib.es2015.symbol.d.ts"),
    (
        "es2015.symbol.wellknown",
        "lib.es2015.symbol.wellknown.d.ts",
    ),
    ("es2016.array.include", "lib.es2016.array.include.d.ts"),
    ("es2016.intl", "lib.es2016.intl.d.ts"),
    ("es2017.arraybuffer", "lib.es2017.arraybuffer.d.ts"),
    ("es2017.date", "lib.es2017.date.d.ts"),
    ("es2017.object", "lib.es2017.object.d.ts"),
    ("es2017.sharedmemory", "lib.es2017.sharedmemory.d.ts"),
    ("es2017.string", "lib.es2017.string.d.ts"),
    ("es2017.intl", "lib.es2017.intl.d.ts"),
    ("es2017.typedarrays", "lib.es2017.typedarrays.d.ts"),
    ("es2018.asyncgenerator", "lib.es2018.asyncgenerator.d.ts"),
    ("es2018.asynciterable", "lib.es2018.asynciterable.d.ts"),
    ("es2018.intl", "lib.es2018.intl.d.ts"),
    ("es2018.promise", "lib.es2018.promise.d.ts"),
    ("es2018.regexp", "lib.es2018.regexp.d.ts"),
    ("es2019.array", "lib.es2019.array.d.ts"),
    ("es2019.object", "lib.es2019.object.d.ts"),
    ("es2019.string", "lib.es2019.string.d.ts"),
    ("es2019.symbol", "lib.es2019.symbol.d.ts"),
    ("es2019.intl", "lib.es2019.intl.d.ts"),
    ("es2020.bigint", "lib.es2020.bigint.d.ts"),
    ("es2020.date", "lib.es2020.date.d.ts"),
    ("es2020.promise", "lib.es2020.promise.d.ts"),
    ("es2020.sharedmemory", "lib.es2020.sharedmemory.d.ts"),
    ("es2020.string", "lib.es2020.string.d.ts"),
    (
        "es2020.symbol.wellknown",
        "lib.es2020.symbol.wellknown.d.ts",
    ),
    ("es2020.intl", "lib.es2020.intl.d.ts"),
    ("es2020.number", "lib.es2020.number.d.ts"),
    ("es2021.promise", "lib.es2021.promise.d.ts"),
    ("es2021.string", "lib.es2021.string.d.ts"),
    ("es2021.weakref", "lib.es2021.weakref.d.ts"),
    ("es2021.intl", "lib.es2021.intl.d.ts"),
    ("es2022.array", "lib.es2022.array.d.ts"),
    ("es2022.error", "lib.es2022.error.d.ts"),
    ("es2022.intl", "lib.es2022.intl.d.ts"),
    ("es2022.object", "lib.es2022.object.d.ts"),
    ("es2022.string", "lib.es2022.string.d.ts"),
    ("es2022.regexp", "lib.es2022.regexp.d.ts"),
    ("es2023.array", "lib.es2023.array.d.ts"),
    ("es2023.collection", "lib.es2023.collection.d.ts"),
    ("es2023.intl", "lib.es2023.intl.d.ts"),
    ("es2024.arraybuffer", "lib.es2024.arraybuffer.d.ts"),
    ("es2024.collection", "lib.es2024.collection.d.ts"),
    ("es2024.object", "lib.es2024.object.d.ts"),
    ("es2024.promise", "lib.es2024.promise.d.ts"),
    ("es2024.regexp", "lib.es2024.regexp.d.ts"),
    ("es2024.sharedmemory", "lib.es2024.sharedmemory.d.ts"),
    ("es2024.string", "lib.es2024.string.d.ts"),
    ("es2025.collection", "lib.es2025.collection.d.ts"),
    ("es2025.float16", "lib.es2025.float16.d.ts"),
    ("es2025.intl", "lib.es2025.intl.d.ts"),
    ("es2025.iterator", "lib.es2025.iterator.d.ts"),
    ("es2025.promise", "lib.es2025.promise.d.ts"),
    ("es2025.regexp", "lib.es2025.regexp.d.ts"),
    ("esnext.asynciterable", "lib.es2018.asynciterable.d.ts"),
    ("esnext.symbol", "lib.es2019.symbol.d.ts"),
    ("esnext.bigint", "lib.es2020.bigint.d.ts"),
    ("esnext.weakref", "lib.es2021.weakref.d.ts"),
    ("esnext.object", "lib.es2024.object.d.ts"),
    ("esnext.regexp", "lib.es2024.regexp.d.ts"),
    ("esnext.string", "lib.es2024.string.d.ts"),
    ("esnext.float16", "lib.es2025.float16.d.ts"),
    ("esnext.iterator", "lib.es2025.iterator.d.ts"),
    ("esnext.promise", "lib.es2025.promise.d.ts"),
    ("esnext.array", "lib.esnext.array.d.ts"),
    ("esnext.collection", "lib.esnext.collection.d.ts"),
    ("esnext.date", "lib.esnext.date.d.ts"),
    ("esnext.decorators", "lib.esnext.decorators.d.ts"),
    ("esnext.disposable", "lib.esnext.disposable.d.ts"),
    ("esnext.error", "lib.esnext.error.d.ts"),
    ("esnext.intl", "lib.esnext.intl.d.ts"),
    ("esnext.sharedmemory", "lib.esnext.sharedmemory.d.ts"),
    ("esnext.temporal", "lib.esnext.temporal.d.ts"),
    ("esnext.typedarrays", "lib.esnext.typedarrays.d.ts"),
    ("decorators", "lib.decorators.d.ts"),
    ("decorators.legacy", "lib.decorators.legacy.d.ts"),
];

static LIB_MAP: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| {
    LIB_ENTRIES
        .iter()
        .map(|&(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
});

pub fn lib_map() -> &'static BTreeMap<String, String> {
    &LIB_MAP
}

pub fn lib_names() -> Vec<String> {
    entry_keys(LIB_ENTRIES)
}

pub fn get_lib_file_name(lib_name: &str) -> Option<String> {
    let lib_name = ts_tspath::to_file_name_lower_case(lib_name);
    if lib_name.starts_with("lib.") && lib_name.ends_with(".d.ts") {
        return Some(lib_name);
    }
    lib_map().get(&lib_name).cloned()
}

static MODULE_RESOLUTION_OPTION_MAP: LazyLock<EnumMap> =
    LazyLock::new(|| enum_map(MODULE_RESOLUTION_ENTRIES));

pub fn module_resolution_option_map() -> &'static EnumMap {
    &MODULE_RESOLUTION_OPTION_MAP
}

const MODULE_RESOLUTION_ENTRIES: &[(&str, &str)] = &[
    ("classic", "Classic"),
    ("node", "Node10"),
    ("node10", "Node10"),
    ("node16", "Node16"),
    ("nodenext", "NodeNext"),
    ("bundler", "Bundler"),
];

static MODULE_OPTION_MAP: LazyLock<EnumMap> = LazyLock::new(|| enum_map(MODULE_ENTRIES));

pub fn module_option_map() -> &'static EnumMap {
    &MODULE_OPTION_MAP
}

const MODULE_ENTRIES: &[(&str, &str)] = &[
    ("commonjs", "CommonJS"),
    ("amd", "AMD"),
    ("system", "System"),
    ("umd", "UMD"),
    ("es6", "ES2015"),
    ("es2015", "ES2015"),
    ("es2020", "ES2020"),
    ("es2022", "ES2022"),
    ("esnext", "ESNext"),
    ("node16", "Node16"),
    ("node18", "Node18"),
    ("node20", "Node20"),
    ("nodenext", "NodeNext"),
    ("preserve", "Preserve"),
];

static TARGET_OPTION_MAP: LazyLock<EnumMap> = LazyLock::new(|| enum_map(TARGET_ENTRIES));

pub fn target_option_map() -> &'static EnumMap {
    &TARGET_OPTION_MAP
}

const TARGET_ENTRIES: &[(&str, &str)] = &[
    ("es3", "ES3"),
    ("es5", "ES5"),
    ("es6", "ES2015"),
    ("es2015", "ES2015"),
    ("es2016", "ES2016"),
    ("es2017", "ES2017"),
    ("es2018", "ES2018"),
    ("es2019", "ES2019"),
    ("es2020", "ES2020"),
    ("es2021", "ES2021"),
    ("es2022", "ES2022"),
    ("es2023", "ES2023"),
    ("es2024", "ES2024"),
    ("es2025", "ES2025"),
    ("esnext", "ESNext"),
];

static TARGET_TO_LIB_MAP: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| {
    [
        ("ESNext", "lib.esnext.full.d.ts"),
        ("ES2025", "lib.es2025.full.d.ts"),
        ("ES2024", "lib.es2024.full.d.ts"),
        ("ES2023", "lib.es2023.full.d.ts"),
        ("ES2022", "lib.es2022.full.d.ts"),
        ("ES2021", "lib.es2021.full.d.ts"),
        ("ES2020", "lib.es2020.full.d.ts"),
        ("ES2019", "lib.es2019.full.d.ts"),
        ("ES2018", "lib.es2018.full.d.ts"),
        ("ES2017", "lib.es2017.full.d.ts"),
        ("ES2016", "lib.es2016.full.d.ts"),
        ("ES2015", "lib.es6.d.ts"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value.to_owned()))
    .collect()
});

pub fn target_to_lib_map() -> &'static BTreeMap<String, String> {
    &TARGET_TO_LIB_MAP
}

pub fn get_default_lib_file_name(options: &ts_core::CompilerOptions) -> String {
    target_to_lib_map()
        .get(&options.get_emit_script_target().to_string())
        .cloned()
        .unwrap_or_else(|| "lib.d.ts".to_owned())
}

static JSX_OPTION_MAP: LazyLock<EnumMap> = LazyLock::new(|| enum_map(JSX_ENTRIES));

pub fn jsx_option_map() -> &'static EnumMap {
    &JSX_OPTION_MAP
}

const JSX_ENTRIES: &[(&str, &str)] = &[
    ("preserve", "Preserve"),
    ("react", "React"),
    ("react-native", "ReactNative"),
    ("react-jsx", "ReactJSX"),
    ("react-jsxdev", "ReactJSXDev"),
];

static MODULE_DETECTION_OPTION_MAP: LazyLock<EnumMap> =
    LazyLock::new(|| enum_map(MODULE_DETECTION_ENTRIES));

pub fn module_detection_option_map() -> &'static EnumMap {
    &MODULE_DETECTION_OPTION_MAP
}

const MODULE_DETECTION_ENTRIES: &[(&str, &str)] =
    &[("legacy", "Legacy"), ("auto", "Auto"), ("force", "Force")];

static NEW_LINE_OPTION_MAP: LazyLock<EnumMap> = LazyLock::new(|| enum_map(NEW_LINE_ENTRIES));

pub fn new_line_option_map() -> &'static EnumMap {
    &NEW_LINE_OPTION_MAP
}

const NEW_LINE_ENTRIES: &[(&str, &str)] = &[("crlf", "CarriageReturnLineFeed"), ("lf", "LineFeed")];

static WATCH_FILE_ENUM_MAP: LazyLock<EnumMap> = LazyLock::new(|| enum_map(WATCH_FILE_ENTRIES));

pub fn watch_file_enum_map() -> &'static EnumMap {
    &WATCH_FILE_ENUM_MAP
}

const WATCH_FILE_ENTRIES: &[(&str, &str)] = &[
    ("fixedpollinginterval", "FixedPollingInterval"),
    ("prioritypollinginterval", "PriorityPollingInterval"),
    ("dynamicprioritypolling", "DynamicPriorityPolling"),
    ("fixedchunksizepolling", "FixedChunkSizePolling"),
    ("usefsevents", "UseFsEvents"),
    (
        "usefseventsonparentdirectory",
        "UseFsEventsOnParentDirectory",
    ),
];

static WATCH_DIRECTORY_ENUM_MAP: LazyLock<EnumMap> =
    LazyLock::new(|| enum_map(WATCH_DIRECTORY_ENTRIES));

pub fn watch_directory_enum_map() -> &'static EnumMap {
    &WATCH_DIRECTORY_ENUM_MAP
}

const WATCH_DIRECTORY_ENTRIES: &[(&str, &str)] = &[
    ("usefsevents", "UseFsEvents"),
    ("fixedpollinginterval", "FixedPollingInterval"),
    ("dynamicprioritypolling", "DynamicPriorityPolling"),
    ("fixedchunksizepolling", "FixedChunkSizePolling"),
];

static FALLBACK_ENUM_MAP: LazyLock<EnumMap> = LazyLock::new(|| enum_map(FALLBACK_ENTRIES));

pub fn fallback_enum_map() -> &'static EnumMap {
    &FALLBACK_ENUM_MAP
}

const FALLBACK_ENTRIES: &[(&str, &str)] = &[
    ("fixedinterval", "FixedInterval"),
    ("priorityinterval", "PriorityInterval"),
    ("dynamicpriority", "DynamicPriority"),
    ("fixedchunksize", "FixedChunkSize"),
];

pub fn enum_keys(option_name: &str) -> Option<Vec<String>> {
    let entries = match option_name {
        "lib" => return Some(lib_names()),
        "moduleResolution" => MODULE_RESOLUTION_ENTRIES,
        "module" => MODULE_ENTRIES,
        "target" => TARGET_ENTRIES,
        "moduleDetection" => MODULE_DETECTION_ENTRIES,
        "jsx" => JSX_ENTRIES,
        "newLine" => NEW_LINE_ENTRIES,
        "watchFile" => WATCH_FILE_ENTRIES,
        "watchDirectory" => WATCH_DIRECTORY_ENTRIES,
        "fallbackPolling" => FALLBACK_ENTRIES,
        _ => return None,
    };
    Some(entry_keys(entries))
}

fn enum_map(entries: &[(&str, &str)]) -> EnumMap {
    entries
        .iter()
        .map(|(key, value)| {
            (
                (*key).to_owned(),
                CompilerOptionsValue::String((*value).to_owned()),
            )
        })
        .collect()
}

fn entry_keys(entries: &[(&str, &str)]) -> Vec<String> {
    entries.iter().map(|(key, _)| (*key).to_owned()).collect()
}

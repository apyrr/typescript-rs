use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Number, Value};
use ts_core::{
    BuildOptions, CompilerOptions, PollingKind, WatchDirectoryKind, WatchFileKind, WatchOptions,
};
use ts_locale::Locale;
use ts_tspath as tspath;

use crate::commandlineparser;
use crate::diagnostics::build_options_did_you_mean_diagnostics;
use crate::namemap::{build_name_map, compiler_name_map};
use crate::parsedcommandline::compiler_option_json_value;
use crate::{
    CommandLineOptionKind, build_opts, command_line_option_enum_map, options_declarations,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedBuildCommandLine {
    pub build_options: BuildOptions,
    pub compiler_options: CompilerOptions,
    pub watch_options: WatchOptions,
    pub projects: Vec<String>,
    pub errors: Vec<String>,
    pub raw: Vec<String>,
    pub current_directory: String,
    pub use_case_sensitive_file_names: bool,
}

impl ParsedBuildCommandLine {
    pub fn resolved_project_paths(&self) -> Vec<String> {
        self.projects
            .iter()
            .map(|project| {
                let resolved = tspath::resolve_path(&self.current_directory, &[project]);
                resolve_config_file_name_of_project_reference(&resolved)
            })
            .collect()
    }

    pub fn locale(&self) -> Locale {
        ts_locale::parse(&self.compiler_options.locale).0
    }

    pub fn compare_paths_options(&self) -> tspath::ComparePathsOptions {
        tspath::ComparePathsOptions {
            current_directory: self.current_directory.clone(),
            use_case_sensitive_file_names: self.use_case_sensitive_file_names,
        }
    }
}

pub fn parse_build_command_line(
    args: &[String],
    current_directory: String,
    use_case_sensitive_file_names: bool,
) -> ParsedBuildCommandLine {
    let build_opts = build_opts();
    let options_declarations = options_declarations();
    let diagnostics = build_options_did_you_mean_diagnostics(
        &build_opts,
        compiler_name_map(options_declarations),
    );
    let parser = commandlineparser::parse_command_line_worker(diagnostics.did_you_mean, args);
    let build_name_map = build_name_map(&build_opts);
    let compiler_name_map = compiler_name_map(options_declarations);

    let mut compiler_json = Map::new();
    let enum_maps = command_line_option_enum_map();
    for (key, value) in &parser.options {
        if parser.explicit_null_options.contains(key) {
            continue;
        }
        let Some(build_option) = build_name_map.get(key) else {
            continue;
        };
        if build_option.name == "build" || compiler_name_map.get(key).is_some() {
            let Some(kind) = build_option.kind else {
                continue;
            };
            if let Some(json_value) = compiler_option_json_value(
                &build_option.name,
                value,
                kind,
                enum_maps.get(&build_option.name),
            ) {
                compiler_json.insert(build_option.name.clone(), json_value);
            }
        }
    }

    let compiler_options = serde_json::from_value::<CompilerOptions>(Value::Object(compiler_json))
        .expect("build command compiler options parsed from command line values");
    let build_options = parse_build_options(&parser.options, &parser.explicit_null_options);
    let watch_options = parse_watch_options(&parser.options, &parser.explicit_null_options);
    let mut projects = parser.file_names;
    if projects.is_empty() {
        projects.push(".".to_owned());
    }

    let mut errors = parser.errors;
    if build_options.clean.is_true() && build_options.force.is_true() {
        errors.push("Options_0_and_1_cannot_be_combined: clean\u{1f}force".to_owned());
    }
    if build_options.clean.is_true() && build_options.verbose.is_true() {
        errors.push("Options_0_and_1_cannot_be_combined: clean\u{1f}verbose".to_owned());
    }
    if build_options.clean.is_true() && compiler_options.watch.is_true() {
        errors.push("Options_0_and_1_cannot_be_combined: clean\u{1f}watch".to_owned());
    }
    if compiler_options.watch.is_true() && build_options.dry.is_true() {
        errors.push("Options_0_and_1_cannot_be_combined: watch\u{1f}dry".to_owned());
    }

    ParsedBuildCommandLine {
        build_options,
        compiler_options,
        watch_options,
        projects,
        errors,
        raw: args.to_vec(),
        current_directory,
        use_case_sensitive_file_names,
    }
}

fn resolve_config_file_name_of_project_reference(project: &str) -> String {
    if project.ends_with(".json") {
        project.to_owned()
    } else {
        tspath::combine_paths(project, &["tsconfig.json"])
    }
}

fn parse_build_options(
    options: &BTreeMap<String, String>,
    explicit_null_options: &BTreeSet<String>,
) -> BuildOptions {
    let mut json = Map::new();
    let build_name_map = build_name_map(&build_opts());
    for (key, value) in options {
        if explicit_null_options.contains(key) {
            continue;
        }
        let Some(option) = build_name_map.get(key) else {
            continue;
        };
        match option.name.as_str() {
            "clean" | "dry" | "force" | "stopBuildOnErrors" | "verbose" => {
                json.insert(option.name.clone(), Value::Bool(value == "true"));
            }
            "builders" => {
                if let Ok(value) = value.parse::<i64>() {
                    json.insert(option.name.clone(), Value::Number(Number::from(value)));
                }
            }
            _ => {}
        }
    }
    serde_json::from_value(Value::Object(json))
        .expect("build options parsed from command line values")
}

fn parse_watch_options(
    options: &BTreeMap<String, String>,
    explicit_null_options: &BTreeSet<String>,
) -> WatchOptions {
    let watch_options = crate::options_for_watch();
    let watch_name_map = crate::watch_name_map(&watch_options);
    let mut json = Map::new();
    for (key, value) in options {
        if explicit_null_options.contains(key) {
            continue;
        }
        let Some(option) = watch_name_map.get(key) else {
            continue;
        };
        let Some(kind) = option.kind else {
            continue;
        };
        let json_value = match kind {
            CommandLineOptionKind::Boolean => Some(Value::Bool(value == "true")),
            CommandLineOptionKind::Number => value
                .parse::<i64>()
                .ok()
                .map(Number::from)
                .map(Value::Number),
            CommandLineOptionKind::String => Some(Value::String(value.clone())),
            CommandLineOptionKind::List | CommandLineOptionKind::ListOrElement => {
                let value = value.trim();
                Some(Value::Array(if value.is_empty() {
                    Vec::new()
                } else {
                    value
                        .split(',')
                        .filter_map(|item| {
                            let item = item.trim();
                            (!item.is_empty()).then(|| Value::String(item.to_owned()))
                        })
                        .collect()
                }))
            }
            CommandLineOptionKind::Enum => watch_enum_option_json_value(&option.name, value),
            CommandLineOptionKind::Object => serde_json::from_str(value).ok(),
        };
        if let Some(json_value) = json_value {
            json.insert(option.name.clone(), json_value);
        }
    }
    serde_json::from_value(Value::Object(json)).unwrap_or_default()
}

fn watch_enum_option_json_value(name: &str, value: &str) -> Option<Value> {
    let value = value.to_ascii_lowercase();
    let numeric = match name {
        "watchFile" => match value.as_str() {
            "fixedpollinginterval" => WatchFileKind::FixedPollingInterval.0,
            "prioritypollinginterval" => WatchFileKind::PriorityPollingInterval.0,
            "dynamicprioritypolling" => WatchFileKind::DynamicPriorityPolling.0,
            "fixedchunksizepolling" => WatchFileKind::FixedChunkSizePolling.0,
            "usefsevents" => WatchFileKind::UseFsEvents.0,
            "usefseventsonparentdirectory" => WatchFileKind::UseFsEventsOnParentDirectory.0,
            _ => return None,
        },
        "watchDirectory" => match value.as_str() {
            "usefsevents" => WatchDirectoryKind::UseFsEvents.0,
            "fixedpollinginterval" => WatchDirectoryKind::FixedPollingInterval.0,
            "dynamicprioritypolling" => WatchDirectoryKind::DynamicPriorityPolling.0,
            "fixedchunksizepolling" => WatchDirectoryKind::FixedChunkSizePolling.0,
            _ => return None,
        },
        "fallbackPolling" => match value.as_str() {
            "fixedinterval" => PollingKind::FixedInterval.0,
            "priorityinterval" => PollingKind::PriorityInterval.0,
            "dynamicpriority" => PollingKind::DynamicPriority.0,
            "fixedchunksize" => PollingKind::FixedChunkSize.0,
            _ => return None,
        },
        _ => return None,
    };
    Some(Value::Number(Number::from(numeric)))
}

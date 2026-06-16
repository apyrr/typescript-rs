use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_locale as locale;
use ts_tsoptions as tsoptions;

use super::{Colors, System, create_colors};

fn string_arg(value: impl Into<String>) -> diagnostics::Argument {
    Box::new(value.into())
}

fn push_example(
    output: &mut Vec<String>,
    colors: &Colors,
    locale: &locale::Locale,
    examples: &[&str],
    desc: &diagnostics::Message,
) {
    for example in examples {
        output.push("  ".to_owned());
        output.push(colors.blue((*example).to_owned()));
        output.push("\n".to_owned());
    }
    output.push("  ".to_owned());
    output.push(desc.localize(locale.clone(), vec![]));
    output.push("\n".to_owned());
    output.push("\n".to_owned());
}

pub fn print_version(mut sys: System, locale: locale::Locale) {
    let _ = writeln!(
        sys.writer(),
        "{}",
        diagnostics::Version_0.localize(locale, vec![string_arg(core::version())])
    );
}

pub fn print_help(
    sys: System,
    locale: locale::Locale,
    command_line: &tsoptions::ParsedCommandLine,
) {
    if command_line.compiler_options().all.is_false_or_unknown() {
        print_easy_help(sys.clone(), locale, get_options_for_help(command_line));
    } else {
        print_all_help(sys.clone(), locale, get_options_for_help(command_line));
    }
}

pub fn get_options_for_help(
    command_line: &tsoptions::ParsedCommandLine,
) -> Vec<tsoptions::CommandLineOption> {
    // Sort our options by their names, (e.g. "--noImplicitAny" comes before "--watch")
    let mut opts = tsoptions::options_declarations().to_vec();
    if let Some(tsc_build_option) = tsoptions::options_for_build()
        .into_iter()
        .find(|option| option.name == "build")
    {
        opts.push(tsc_build_option);
    }

    if command_line.compiler_options().all.is_true() {
        opts.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        opts
    } else {
        let mut opts: Vec<_> = opts
            .into_iter()
            .filter(|opt| opt.show_in_simplified_help_view)
            .collect();
        opts.sort_by_key(|opt| simplified_help_order(&opt.name));
        opts
    }
}

fn simplified_help_order(name: &str) -> usize {
    [
        "help",
        "watch",
        "all",
        "version",
        "init",
        "project",
        "showConfig",
        "ignoreConfig",
        "build",
        "pretty",
        "incremental",
        "declaration",
        "declarationMap",
        "emitDeclarationOnly",
        "sourceMap",
        "noEmit",
        "target",
        "module",
        "lib",
        "allowJs",
        "checkJs",
        "jsx",
        "outFile",
        "outDir",
        "removeComments",
        "strict",
        "types",
        "esModuleInterop",
    ]
    .iter()
    .position(|known| known == &name)
    .unwrap_or(usize::MAX)
}

pub fn get_header(sys: System, message: String) -> Vec<String> {
    let colors = create_colors(sys.clone());
    let mut header = Vec::with_capacity(3);
    let terminal_width = sys.get_width_of_terminal();
    const TS_ICON: &str = "     ";
    const TS_ICON_TS: &str = "  TS ";
    let ts_icon_length = TS_ICON.len();

    let ts_icon_first_line = colors.blue_background(TS_ICON.to_owned());
    let ts_icon_second_line = colors.blue_background(colors.bright_white(TS_ICON_TS.to_owned()));
    // If we have enough space, print TS icon.
    if terminal_width as usize >= message.len() + ts_icon_length {
        // right align of the icon is 120 at most.
        let right_align = core::if_else(terminal_width > 120, 120, terminal_width) as usize;
        let left_align = right_align - ts_icon_length;
        header.push(format!("{message:<left_align$}"));
        header.push(ts_icon_first_line);
        header.push("\n".to_owned());
        header.push(" ".repeat(left_align));
        header.push(ts_icon_second_line);
        header.push("\n".to_owned());
    } else {
        header.push(message);
        header.push("\n".to_owned());
        header.push("\n".to_owned());
    }
    header
}

pub fn print_easy_help(
    mut sys: System,
    locale: locale::Locale,
    simple_options: Vec<tsoptions::CommandLineOption>,
) {
    let colors = create_colors(sys.clone());
    let mut output = Vec::new();

    let msg = format!(
        "{} - {}",
        diagnostics::X_tsc_Colon_The_TypeScript_Compiler.localize(locale.clone(), vec![]),
        diagnostics::Version_0.localize(locale.clone(), vec![string_arg(core::version())])
    );
    output.extend(get_header(sys.clone(), msg));

    output.push(colors.bold(diagnostics::COMMON_COMMANDS.localize(locale.clone(), vec![])));
    output.push("\n".to_owned());
    output.push("\n".to_owned());

    push_example(
        &mut output,
        &colors,
        &locale,
        &["tsc"],
        &diagnostics::Compiles_the_current_project_tsconfig_json_in_the_working_directory,
    );
    push_example(
        &mut output,
        &colors,
        &locale,
        &["tsc app.ts util.ts"],
        &diagnostics::Ignoring_tsconfig_json_compiles_the_specified_files_with_default_compiler_options,
    );
    push_example(
        &mut output,
        &colors,
        &locale,
        &["tsc -b"],
        &diagnostics::Build_a_composite_project_in_the_working_directory,
    );
    push_example(
        &mut output,
        &colors,
        &locale,
        &["tsc --init"],
        &diagnostics::Creates_a_tsconfig_json_with_the_recommended_settings_in_the_working_directory,
    );
    push_example(
        &mut output,
        &colors,
        &locale,
        &["tsc -p ./path/to/tsconfig.json"],
        &diagnostics::Compiles_the_TypeScript_project_located_at_the_specified_path,
    );
    push_example(
        &mut output,
        &colors,
        &locale,
        &["tsc --help --all"],
        &diagnostics::An_expanded_version_of_this_information_showing_all_possible_compiler_options,
    );
    push_example(
        &mut output,
        &colors,
        &locale,
        &["tsc --noEmit", "tsc --target esnext"],
        &diagnostics::Compiles_the_current_project_with_additional_settings,
    );

    let mut cli_commands = Vec::new();
    let mut config_opts = Vec::new();
    for opt in simple_options {
        if opt.is_command_line_only
            || is_command_line_options_category(opt.category.as_deref(), locale.clone())
        {
            cli_commands.push(opt);
        } else {
            config_opts.push(opt);
        }
    }

    output.extend(generate_section_options_output(
        sys.clone(),
        locale.clone(),
        diagnostics::COMMAND_LINE_FLAGS.localize(locale.clone(), vec![]),
        cli_commands,
        false,
        None,
        None,
    ));

    let after = diagnostics::You_can_learn_about_all_of_the_compiler_options_at_0
        .localize(locale.clone(), vec![string_arg("https://aka.ms/tsc")]);
    output.extend(generate_section_options_output(
        sys.clone(),
        locale.clone(),
        diagnostics::COMMON_COMPILER_OPTIONS.localize(locale.clone(), vec![]),
        config_opts,
        false,
        None,
        Some(after),
    ));

    for chunk in output {
        let _ = write!(sys.writer(), "{chunk}");
    }
}

pub fn print_all_help(
    mut sys: System,
    locale: locale::Locale,
    options: Vec<tsoptions::CommandLineOption>,
) {
    let mut output = Vec::new();
    let msg = format!(
        "{} - {}",
        diagnostics::X_tsc_Colon_The_TypeScript_Compiler.localize(locale.clone(), vec![]),
        diagnostics::Version_0.localize(locale.clone(), vec![string_arg(core::version())])
    );
    output.extend(get_header(sys.clone(), msg));

    // ALL COMPILER OPTIONS section
    let after_compiler_options = diagnostics::You_can_learn_about_all_of_the_compiler_options_at_0
        .localize(locale.clone(), vec![string_arg("https://aka.ms/tsc")]);
    output.extend(generate_section_options_output(
        sys.clone(),
        locale.clone(),
        diagnostics::ALL_COMPILER_OPTIONS.localize(locale.clone(), vec![]),
        options,
        true,
        None,
        Some(after_compiler_options),
    ));

    // WATCH OPTIONS section
    let before_watch_options = diagnostics::Including_watch_w_will_start_watching_the_current_project_for_the_file_changes_Once_set_you_can_config_watch_mode_with_Colon.localize(locale.clone(), vec![]);
    output.extend(generate_section_options_output(
        sys.clone(),
        locale.clone(),
        diagnostics::WATCH_OPTIONS.localize(locale.clone(), vec![]),
        tsoptions::options_for_watch(),
        false,
        Some(before_watch_options),
        None,
    ));

    // BUILD OPTIONS section
    let before_build_options = diagnostics::Using_build_b_will_make_tsc_behave_more_like_a_build_orchestrator_than_a_compiler_This_is_used_to_trigger_building_composite_projects_which_you_can_learn_more_about_at_0.localize(locale.clone(), vec![string_arg("https://aka.ms/tsc-composite-builds")]);
    let build_options = options_for_build_help(tsoptions::options_for_build());
    output.extend(generate_section_options_output(
        sys.clone(),
        locale.clone(),
        diagnostics::BUILD_OPTIONS.localize(locale.clone(), vec![]),
        build_options,
        false,
        Some(before_build_options),
        None,
    ));

    for chunk in output {
        let _ = write!(sys.writer(), "{chunk}");
    }
}

pub fn print_build_help(
    mut sys: System,
    locale: locale::Locale,
    build_options: Vec<tsoptions::CommandLineOption>,
) {
    let mut output = Vec::new();
    output.extend(get_header(
        sys.clone(),
        diagnostics::X_tsc_Colon_The_TypeScript_Compiler.localize(locale.clone(), vec![])
            + " - "
            + &diagnostics::Version_0.localize(locale.clone(), vec![string_arg(core::version())]),
    ));
    let before = diagnostics::Using_build_b_will_make_tsc_behave_more_like_a_build_orchestrator_than_a_compiler_This_is_used_to_trigger_building_composite_projects_which_you_can_learn_more_about_at_0.localize(locale.clone(), vec![string_arg("https://aka.ms/tsc-composite-builds")]);
    let options = options_for_build_help(build_options);
    output.extend(generate_section_options_output(
        sys.clone(),
        locale.clone(),
        diagnostics::BUILD_OPTIONS.localize(locale.clone(), vec![]),
        options,
        false,
        Some(before),
        None,
    ));

    for chunk in output {
        let _ = write!(sys.writer(), "{chunk}");
    }
}

fn options_for_build_help(
    build_options: Vec<tsoptions::CommandLineOption>,
) -> Vec<tsoptions::CommandLineOption> {
    build_options
        .into_iter()
        .filter(|option| option.name != "build")
        .collect()
}

pub fn generate_section_options_output(
    sys: System,
    locale: locale::Locale,
    section_name: String,
    options: Vec<tsoptions::CommandLineOption>,
    sub_category: bool,
    before_options_description: Option<String>,
    after_options_description: Option<String>,
) -> Vec<String> {
    let mut output = vec![
        create_colors(sys.clone()).bold(section_name),
        "\n".to_owned(),
        "\n".to_owned(),
    ];

    if let Some(before_options_description) = before_options_description {
        output.push(before_options_description);
        output.push("\n".to_owned());
        output.push("\n".to_owned());
    }
    if !sub_category {
        output.extend(generate_group_option_output(sys, locale, options));
        if let Some(after_options_description) = after_options_description {
            output.push(after_options_description);
            output.push("\n".to_owned());
            output.push("\n".to_owned());
        }
        return output;
    }
    let mut category_map: Vec<(String, Vec<tsoptions::CommandLineOption>)> = Vec::new();
    for option in options {
        let Some(category) = option.category.as_deref() else {
            continue;
        };
        let cur_category = localize_option_category(category, locale.clone());
        if let Some((_, options)) = category_map
            .iter_mut()
            .find(|(category, _)| category == &cur_category)
        {
            options.push(option);
        } else {
            category_map.push((cur_category, vec![option]));
        }
    }
    for (key, value) in category_map {
        output.push("### ".to_owned());
        output.push(key);
        output.push("\n".to_owned());
        output.push("\n".to_owned());
        output.extend(generate_group_option_output(
            sys.clone(),
            locale.clone(),
            value,
        ));
    }
    if let Some(after_options_description) = after_options_description {
        output.push(after_options_description);
        output.push("\n".to_owned());
        output.push("\n".to_owned());
    }

    output
}

pub fn generate_group_option_output(
    sys: System,
    locale: locale::Locale,
    options_list: Vec<tsoptions::CommandLineOption>,
) -> Vec<String> {
    let mut max_length = 0;
    for option in &options_list {
        let cur_length = get_display_name_text_of_option(option).len();
        max_length = max_length.max(cur_length);
    }

    // left part should be right align, right part should be left align

    // assume 2 space between left margin and left part.
    let right_align_of_left_part = max_length + 2;
    // assume 2 space between left and right part
    let left_align_of_right_part = right_align_of_left_part + 2;

    let mut lines = Vec::new();
    for option in &options_list {
        let tmp = generate_option_output(
            sys.clone(),
            locale.clone(),
            option,
            right_align_of_left_part,
            left_align_of_right_part,
        );
        lines.extend(tmp);
    }

    // make sure always a blank line in the end.
    if lines.len() < 2 || lines[lines.len() - 2] != "\n" {
        lines.push("\n".to_owned());
    }

    lines
}

pub fn generate_option_output(
    sys: System,
    locale: locale::Locale,
    option: &tsoptions::CommandLineOption,
    right_align_of_left: usize,
    left_align_of_right: usize,
) -> Vec<String> {
    let mut text = Vec::new();
    let colors = create_colors(sys.clone());

    // name and description
    let name = get_display_name_text_of_option(option);

    // value type and possible value
    let value_candidates = get_value_candidate(sys.clone(), locale.clone(), option);

    let default_value_description = format_default_value(
        option.default_value_description.clone(),
        if option.kind == Some(tsoptions::CommandLineOptionKind::List)
            || option.kind == Some(tsoptions::CommandLineOptionKind::ListOrElement)
        {
            option.elements()
        } else {
            Some(option.clone())
        },
    );

    let terminal_width = sys.get_width_of_terminal() as usize;

    if terminal_width >= 80 {
        let description = option.description.clone().unwrap_or_default();
        text.extend(get_pretty_output(
            &colors,
            name,
            description,
            right_align_of_left,
            left_align_of_right,
            terminal_width,
            true,
        ));
        text.push("\n".to_owned());
        if show_additional_info_output(value_candidates.as_ref(), option, locale.clone()) {
            if let Some(value_candidates) = &value_candidates {
                text.extend(get_pretty_output(
                    &colors,
                    value_candidates.value_type.clone(),
                    value_candidates.possible_values.clone(),
                    right_align_of_left,
                    left_align_of_right,
                    terminal_width,
                    false,
                ));
                text.push("\n".to_owned());
            }
            if !default_value_description.is_empty() {
                text.extend(get_pretty_output(
                    &colors,
                    diagnostics::X_default_Colon.localize(locale, vec![]),
                    default_value_description,
                    right_align_of_left,
                    left_align_of_right,
                    terminal_width,
                    false,
                ));
                text.push("\n".to_owned());
            }
        }
        text.push("\n".to_owned());
    } else {
        text.push(colors.blue(name));
        text.push("\n".to_owned());
        if let Some(description) = &option.description {
            text.push(description.clone());
        }
        text.push("\n".to_owned());
        if show_additional_info_output(value_candidates.as_ref(), option, locale.clone()) {
            if let Some(value_candidates) = &value_candidates {
                text.push(value_candidates.value_type.clone());
                text.push(" ".to_owned());
                text.push(value_candidates.possible_values.clone());
            }
            if !default_value_description.is_empty() {
                if value_candidates.is_some() {
                    text.push("\n".to_owned());
                }
                text.push(diagnostics::X_default_Colon.localize(locale, vec![]));
                text.push(" ".to_owned());
                text.push(default_value_description);
            }

            text.push("\n".to_owned());
        }
        text.push("\n".to_owned());
    }

    text
}

pub fn format_default_value(
    default_value: Option<tsoptions::DefaultValueDescription>,
    option: Option<tsoptions::CommandLineOption>,
) -> String {
    let Some(default_value) = default_value else {
        return "undefined".to_owned();
    };
    if default_value == tsoptions::DefaultValueDescription::Unknown {
        return "undefined".to_owned();
    }

    if option
        .as_ref()
        .is_some_and(|option| option.kind == Some(tsoptions::CommandLineOptionKind::Enum))
    {
        // e.g. ScriptTarget.ES2015 -> "es6/es2015"
        let mut names = Vec::new();
        if let Some(option) = &option {
            for (name, value) in enum_map_entries(option) {
                if compiler_options_value_matches_default(&value, &default_value) {
                    names.push(name);
                }
            }
        }
        if !names.is_empty() {
            return names.join("/");
        }
        if matches!(
            option.as_ref().map(|option| option.name.as_str()),
            Some("moduleDetection" | "moduleResolution")
        ) && let tsoptions::DefaultValueDescription::String(value) = default_value
        {
            return value;
        }
        return String::new();
    }

    match default_value {
        tsoptions::DefaultValueDescription::Bool(value) => value.to_string(),
        tsoptions::DefaultValueDescription::String(value) => value,
        tsoptions::DefaultValueDescription::Number(value) => value.to_string(),
        tsoptions::DefaultValueDescription::Unknown => "undefined".to_owned(),
    }
}

fn compiler_options_value_matches_default(
    value: &tsoptions::CompilerOptionsValue,
    default_value: &tsoptions::DefaultValueDescription,
) -> bool {
    matches!(
        (value, default_value),
        (
            tsoptions::CompilerOptionsValue::Bool(value),
            tsoptions::DefaultValueDescription::Bool(default_value)
        ) if value == default_value
    ) || matches!(
        (value, default_value),
        (
            tsoptions::CompilerOptionsValue::String(value),
            tsoptions::DefaultValueDescription::String(default_value)
        ) if value == default_value
    ) || matches!(
        (value, default_value),
        (
            tsoptions::CompilerOptionsValue::Number(value),
            tsoptions::DefaultValueDescription::Number(default_value)
        ) if value == default_value
    ) || matches!(
        (value, default_value),
        (
            tsoptions::CompilerOptionsValue::Unknown,
            tsoptions::DefaultValueDescription::Unknown
        )
    )
}

fn is_command_line_options_category(category: Option<&str>, locale: locale::Locale) -> bool {
    if let Some(category) = category {
        category == diagnostics::Command_line_Options.string()
            || category == diagnostics::Command_line_Options.localize(locale, vec![])
    } else {
        false
    }
}

fn localize_option_category(category: &str, locale: locale::Locale) -> String {
    match category {
        category if category == diagnostics::Command_line_Options.string() => {
            diagnostics::Command_line_Options.localize(locale, vec![])
        }
        category if category == diagnostics::Modules.string() => {
            diagnostics::Modules.localize(locale, vec![])
        }
        category if category == diagnostics::File_Management.string() => {
            diagnostics::File_Management.localize(locale, vec![])
        }
        category if category == diagnostics::Emit.string() => {
            diagnostics::Emit.localize(locale, vec![])
        }
        category if category == diagnostics::JavaScript_Support.string() => {
            diagnostics::JavaScript_Support.localize(locale, vec![])
        }
        category if category == diagnostics::Type_Checking.string() => {
            diagnostics::Type_Checking.localize(locale, vec![])
        }
        category if category == diagnostics::Editor_Support.string() => {
            diagnostics::Editor_Support.localize(locale, vec![])
        }
        category if category == diagnostics::Watch_and_Build_Modes.string() => {
            diagnostics::Watch_and_Build_Modes.localize(locale, vec![])
        }
        category if category == diagnostics::Compiler_Diagnostics.string() => {
            diagnostics::Compiler_Diagnostics.localize(locale, vec![])
        }
        category if category == diagnostics::Interop_Constraints.string() => {
            diagnostics::Interop_Constraints.localize(locale, vec![])
        }
        category if category == diagnostics::Backwards_Compatibility.string() => {
            diagnostics::Backwards_Compatibility.localize(locale, vec![])
        }
        category if category == diagnostics::Language_and_Environment.string() => {
            diagnostics::Language_and_Environment.localize(locale, vec![])
        }
        category if category == diagnostics::Projects.string() => {
            diagnostics::Projects.localize(locale, vec![])
        }
        category if category == diagnostics::Output_Formatting.string() => {
            diagnostics::Output_Formatting.localize(locale, vec![])
        }
        category if category == diagnostics::Completeness.string() => {
            diagnostics::Completeness.localize(locale, vec![])
        }
        _ => category.to_owned(),
    }
}

fn default_value_description_is_empty_false_or_na(
    default_value_description: Option<&tsoptions::DefaultValueDescription>,
) -> bool {
    match default_value_description {
        None => true,
        Some(tsoptions::DefaultValueDescription::String(value)) => {
            value == "false" || value == "n/a"
        }
        _ => false,
    }
}

pub struct ValueCandidate {
    // "one or more" or "any of"
    pub value_type: String,
    pub possible_values: String,
}

pub fn show_additional_info_output(
    value_candidates: Option<&ValueCandidate>,
    option: &tsoptions::CommandLineOption,
    locale: locale::Locale,
) -> bool {
    if is_command_line_options_category(option.category.as_deref(), locale) {
        return false;
    }
    if let Some(value_candidates) = value_candidates {
        if value_candidates.possible_values == "string"
            && default_value_description_is_empty_false_or_na(
                option.default_value_description.as_ref(),
            )
        {
            return false;
        }
    }
    true
}

pub fn get_value_candidate(
    _sys: System,
    locale: locale::Locale,
    option: &tsoptions::CommandLineOption,
) -> Option<ValueCandidate> {
    // option.type might be "string" | "number" | "boolean" | "object" | "list" | Map<string, number | string>
    // string -- any of: string
    // number -- any of: number
    // boolean -- any of: boolean
    // object -- null
    // list -- one or more: , content depends on `option.element.type`, the same as others
    // Map<string, number | string> -- any of: key1, key2, ....
    if option.kind == Some(tsoptions::CommandLineOptionKind::Object) {
        return None;
    }

    let mut res = ValueCandidate {
        value_type: String::new(),
        possible_values: String::new(),
    };
    if option.kind == Some(tsoptions::CommandLineOptionKind::ListOrElement) {
        // assert(option.type !== "listOrElement")
        panic!("no value candidate for list or element");
    }

    match option.kind {
        Some(tsoptions::CommandLineOptionKind::String)
        | Some(tsoptions::CommandLineOptionKind::Number)
        | Some(tsoptions::CommandLineOptionKind::Boolean) => {
            res.value_type = diagnostics::X_type_Colon.localize(locale, vec![])
        }
        Some(tsoptions::CommandLineOptionKind::List) => {
            res.value_type = diagnostics::X_one_or_more_Colon.localize(locale, vec![])
        }
        _ => res.value_type = diagnostics::X_one_of_Colon.localize(locale, vec![]),
    }

    res.possible_values = get_possible_values(option);

    Some(res)
}

pub fn get_possible_values(option: &tsoptions::CommandLineOption) -> String {
    match option.kind {
        Some(tsoptions::CommandLineOptionKind::String)
        | Some(tsoptions::CommandLineOptionKind::Number)
        | Some(tsoptions::CommandLineOptionKind::Boolean) => option
            .kind
            .map(|kind| kind.as_str().to_owned())
            .unwrap_or_default(),
        Some(tsoptions::CommandLineOptionKind::List)
        | Some(tsoptions::CommandLineOptionKind::ListOrElement) => option
            .elements()
            .as_ref()
            .map(get_possible_values)
            .unwrap_or_default(),
        Some(tsoptions::CommandLineOptionKind::Object) => String::new(),
        _ => {
            // Map<string, number | string>
            // Group synonyms: es6/es2015
            let enum_map = enum_map_entries(option);
            if enum_map.is_empty() {
                return String::new();
            };
            let deprecated_keys = option.deprecated_keys();
            let mut inverted: Vec<(tsoptions::CompilerOptionsValue, Vec<String>)> = Vec::new();

            for (name, value) in enum_map {
                if deprecated_keys
                    .as_ref()
                    .is_none_or(|keys| !keys.contains(&name))
                {
                    if let Some((_, synonyms)) =
                        inverted.iter_mut().find(|(existing, _)| existing == &value)
                    {
                        synonyms.push(name);
                    } else {
                        inverted.push((value, vec![name]));
                    }
                }
            }
            let mut syns = Vec::new();
            for (_, synonyms) in inverted {
                syns.push(synonyms.join("/"));
            }
            syns.join(", ")
        }
    }
}

fn enum_map_entries(
    option: &tsoptions::CommandLineOption,
) -> Vec<(String, tsoptions::CompilerOptionsValue)> {
    if option.kind != Some(tsoptions::CommandLineOptionKind::Enum) {
        return Vec::new();
    }

    let entries: Option<&[(&str, &str)]> = match option.name.as_str() {
        "lib" => Some(&[
            // JavaScript only
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
            // Host only
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
            // ES2015 and later By-feature options
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
            // Fallback for backward compatibility
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
            // ESNext By-feature options
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
            // Decorators
            ("decorators", "lib.decorators.d.ts"),
            ("decorators.legacy", "lib.decorators.legacy.d.ts"),
        ]),
        "moduleResolution" => Some(&[
            ("node16", "Node16"),
            ("nodenext", "NodeNext"),
            ("bundler", "Bundler"),
            ("classic", "Classic"),
            ("node", "Node10"),
            ("node10", "Node10"),
        ]),
        "module" => Some(&[
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
        ]),
        "target" => Some(&[
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
        ]),
        "moduleDetection" => Some(&[("auto", "Auto"), ("legacy", "Legacy"), ("force", "Force")]),
        "jsx" => Some(&[
            ("preserve", "Preserve"),
            ("react-native", "ReactNative"),
            ("react-jsx", "ReactJSX"),
            ("react-jsxdev", "ReactJSXDev"),
            ("react", "React"),
        ]),
        "newLine" => Some(&[("crlf", "CarriageReturnLineFeed"), ("lf", "LineFeed")]),
        "watchFile" => Some(&[
            ("fixedpollinginterval", "FixedPollingInterval"),
            ("prioritypollinginterval", "PriorityPollingInterval"),
            ("dynamicprioritypolling", "DynamicPriorityPolling"),
            ("fixedchunksizepolling", "FixedChunkSizePolling"),
            ("usefsevents", "UseFsEvents"),
            (
                "usefseventsonparentdirectory",
                "UseFsEventsOnParentDirectory",
            ),
        ]),
        "watchDirectory" => Some(&[
            ("usefsevents", "UseFsEvents"),
            ("fixedpollinginterval", "FixedPollingInterval"),
            ("dynamicprioritypolling", "DynamicPriorityPolling"),
            ("fixedchunksizepolling", "FixedChunkSizePolling"),
        ]),
        "fallbackPolling" => Some(&[
            ("fixedinterval", "FixedInterval"),
            ("priorityinterval", "PriorityInterval"),
            ("dynamicpriority", "DynamicPriority"),
            ("fixedchunksize", "FixedChunkSize"),
        ]),
        _ => None,
    };

    entries.map_or_else(
        || {
            option
                .enum_map()
                .map(|enum_map| {
                    enum_map
                        .iter()
                        .map(|(name, value)| (name.clone(), value.clone()))
                        .collect()
                })
                .unwrap_or_default()
        },
        |entries| {
            entries
                .iter()
                .map(|(name, value)| {
                    (
                        (*name).to_owned(),
                        tsoptions::CompilerOptionsValue::String((*value).to_owned()),
                    )
                })
                .collect()
        },
    )
}

pub fn get_pretty_output(
    colors: &Colors,
    left: String,
    right: String,
    right_align_of_left: usize,
    left_align_of_right: usize,
    terminal_width: usize,
    color_left: bool,
) -> Vec<String> {
    // !!! How does terminalWidth interact with UTF-8 encoding? Strada just assumed UTF-16.
    let mut res = Vec::with_capacity(4);
    let mut is_first_line = true;
    let mut remain_right = right;
    let right_character_number = terminal_width - left_align_of_right;
    while !remain_right.is_empty() {
        let mut cur_left = String::new();
        if is_first_line {
            cur_left = format!("{left:>right_align_of_left$}");
            cur_left = format!("{cur_left:<left_align_of_right$}");
            if color_left {
                cur_left = colors.blue(cur_left);
            }
        } else {
            cur_left = " ".repeat(left_align_of_right);
        }

        let idx = right_character_number.min(remain_right.len());
        let cur_right = remain_right[..idx].to_owned();
        remain_right = remain_right[idx..].to_owned();
        res.push(cur_left);
        res.push(cur_right);
        res.push("\n".to_owned());
        is_first_line = false;
    }
    res
}

pub fn get_display_name_text_of_option(option: &tsoptions::CommandLineOption) -> String {
    format!(
        "--{}{}",
        option.name,
        core::if_else(
            !option.short_name.is_empty(),
            format!(", -{}", option.short_name),
            String::new()
        )
    )
}

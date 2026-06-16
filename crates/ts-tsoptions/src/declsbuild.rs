use crate::{
    CommandLineOption, CommandLineOptionKind, DefaultValueDescription, ExtraValidation, Tristate,
};

fn bool_option(
    name: &str,
    short_name: &str,
    category: &str,
    description: &str,
    show_in_simplified_help_view: bool,
    is_command_line_only: bool,
    default_value_description: bool,
) -> CommandLineOption {
    CommandLineOption {
        name: name.to_owned(),
        short_name: short_name.to_owned(),
        kind: Some(CommandLineOptionKind::Boolean),
        show_in_simplified_help_view,
        is_command_line_only,
        category: Some(category.to_owned()),
        description: if description.is_empty() {
            None
        } else {
            Some(description.to_owned())
        },
        default_value_description: Some(DefaultValueDescription::Bool(default_value_description)),
        ..CommandLineOption::default()
    }
}

fn string_option(
    name: &str,
    category: &str,
    description: &str,
    default: Option<&str>,
) -> CommandLineOption {
    CommandLineOption {
        name: name.to_owned(),
        kind: Some(CommandLineOptionKind::String),
        is_file_path: true,
        category: Some(category.to_owned()),
        description: Some(description.to_owned()),
        default_value_description: default
            .map(|value| DefaultValueDescription::String(value.to_owned())),
        ..CommandLineOption::default()
    }
}

fn with_default(
    mut option: CommandLineOption,
    default_value_description: DefaultValueDescription,
) -> CommandLineOption {
    option.default_value_description = Some(default_value_description);
    option
}

pub fn common_options_with_build() -> Vec<CommandLineOption> {
    let command_line = "Command-line Options";
    let output_formatting = "Output Formatting";
    let compiler_diagnostics = "Compiler Diagnostics";
    let emit = "Emit";
    let projects = "Projects";
    let type_checking = "Type Checking";
    let watch_and_build_modes = "Watch and Build Modes";

    vec![
        //******* commonOptionsWithBuild *******
        bool_option(
            "help",
            "h",
            command_line,
            "Print this message.",
            true,
            true,
            false,
        ),
        bool_option("help", "?", command_line, "", false, true, false),
        bool_option(
            "watch",
            "w",
            command_line,
            "Watch input files.",
            true,
            true,
            false,
        ),
        bool_option(
            "preserveWatchOutput",
            "",
            output_formatting,
            "Disable wiping the console in watch mode.",
            false,
            false,
            false,
        ),
        bool_option(
            "listFiles",
            "",
            compiler_diagnostics,
            "Print all of the files read during the compilation.",
            false,
            false,
            false,
        ),
        bool_option(
            "explainFiles",
            "",
            compiler_diagnostics,
            "Print files read during the compilation including why it was included.",
            false,
            false,
            false,
        ),
        bool_option(
            "listEmittedFiles",
            "",
            compiler_diagnostics,
            "Print the names of emitted files after a compilation.",
            false,
            false,
            false,
        ),
        bool_option(
            "pretty",
            "",
            output_formatting,
            "Enable color and formatting in TypeScript's output to make compiler errors easier to read.",
            true,
            false,
            true,
        ),
        bool_option(
            "traceResolution",
            "",
            compiler_diagnostics,
            "Log paths used during the 'moduleResolution' process.",
            false,
            false,
            false,
        ),
        bool_option(
            "diagnostics",
            "",
            compiler_diagnostics,
            "Output compiler performance information after building.",
            false,
            false,
            false,
        ),
        bool_option(
            "extendedDiagnostics",
            "",
            compiler_diagnostics,
            "Output more detailed compiler performance information after building.",
            false,
            false,
            false,
        ),
        string_option(
            "generateCpuProfile",
            compiler_diagnostics,
            "Emit a v8 CPU profile of the compiler run for debugging.",
            Some("profile.cpuprofile"),
        ),
        string_option(
            "generateTrace",
            compiler_diagnostics,
            "Generates an event trace and a list of types.",
            None,
        ),
        {
            let mut option = with_default(
                bool_option(
                    "incremental",
                    "i",
                    projects,
                    "Save .tsbuildinfo files to allow for incremental compilation of projects.",
                    false,
                    false,
                    false,
                ),
                DefaultValueDescription::String("`false`, unless `composite` is set".to_owned()),
            );
            option.transpile_option_value = Tristate::Unknown;
            option
        },
        {
            let mut option = with_default(
                bool_option(
                    "declaration",
                    "d",
                    emit,
                    "Generate .d.ts files from TypeScript and JavaScript files in your project.",
                    true,
                    false,
                    false,
                ),
                DefaultValueDescription::String("`false`, unless `composite` is set".to_owned()),
            );
            // Not setting affectsEmit because we calculate this flag might not affect full emit
            option.affects_build_info = true;
            option.transpile_option_value = Tristate::Unknown;
            option
        },
        {
            let mut option = bool_option(
                "declarationMap",
                "",
                emit,
                "Create sourcemaps for d.ts files.",
                true,
                false,
                false,
            );
            // Not setting affectsEmit because we calculate this flag might not affect full emit
            option.affects_build_info = true;
            option
        },
        {
            let mut option = bool_option(
                "emitDeclarationOnly",
                "",
                emit,
                "Only output d.ts files and not JavaScript files.",
                true,
                false,
                false,
            );
            // Not setting affectsEmit because we calculate this flag might not affect full emit
            option.affects_build_info = true;
            option.transpile_option_value = Tristate::Unknown;
            option
        },
        {
            let mut option = bool_option(
                "sourceMap",
                "",
                emit,
                "Create source map files for emitted JavaScript files.",
                true,
                false,
                false,
            );
            // Not setting affectsEmit because we calculate this flag might not affect full emit
            option.affects_build_info = true;
            option
        },
        {
            let mut option = bool_option(
                "inlineSourceMap",
                "",
                emit,
                "Include sourcemap files inside the emitted JavaScript.",
                false,
                false,
                false,
            );
            // Not setting affectsEmit because we calculate this flag might not affect full emit
            option.affects_build_info = true;
            option
        },
        {
            let mut option = bool_option(
                "noCheck",
                "",
                "Compiler Diagnostics",
                "Disable full type checking (only critical parse and emit errors will be reported).",
                false,
                false,
                false,
            );
            option.transpile_option_value = Tristate::True;
            // Not setting affectsSemanticDiagnostics or affectsBuildInfo because we dont want all diagnostics to go away, its handled in builder
            option
        },
        {
            let mut option = bool_option(
                "deduplicatePackages",
                "",
                type_checking,
                "Deduplicate packages with the same name and version.",
                false,
                false,
                true,
            );
            option.affects_program_structure = true;
            option
        },
        {
            let mut option = bool_option(
                "noEmit",
                "",
                emit,
                "Disable emitting files from a compilation.",
                true,
                false,
                false,
            );
            option.transpile_option_value = Tristate::Unknown;
            option
        },
        {
            let mut option = bool_option(
                "assumeChangesOnlyAffectDirectDependencies",
                "",
                watch_and_build_modes,
                "Have recompiles in projects that use 'incremental' and 'watch' mode assume that changes within a file will only affect files directly depending on it.",
                false,
                false,
                false,
            );
            option.affects_semantic_diagnostics = true;
            option.affects_emit = true;
            option.affects_build_info = true;
            option
        },
        CommandLineOption {
            name: "locale".to_owned(),
            kind: Some(CommandLineOptionKind::String),
            is_command_line_only: true,
            category: Some(command_line.to_owned()),
            description: Some(
                "Set the language of the messaging from TypeScript. This does not affect emit."
                    .to_owned(),
            ),
            default_value_description: Some(DefaultValueDescription::String(
                "Platform specific".to_owned(),
            )),
            extra_validation: ExtraValidation::Locale,
            ..CommandLineOption::default()
        },
        CommandLineOption {
            name: "quiet".to_owned(),
            short_name: "q".to_owned(),
            kind: Some(CommandLineOptionKind::Boolean),
            category: Some(command_line.to_owned()),
            description: Some("Do not print diagnostics.".to_owned()),
            ..CommandLineOption::default()
        },
        CommandLineOption {
            name: "singleThreaded".to_owned(),
            kind: Some(CommandLineOptionKind::Boolean),
            category: Some(command_line.to_owned()),
            description: Some("Run in single threaded mode.".to_owned()),
            ..CommandLineOption::default()
        },
        CommandLineOption {
            name: "pprofDir".to_owned(),
            kind: Some(CommandLineOptionKind::String),
            is_file_path: true,
            category: Some(command_line.to_owned()),
            description: Some(
                "Generate pprof CPU/memory profiles to the given directory.".to_owned(),
            ),
            ..CommandLineOption::default()
        },
        CommandLineOption {
            name: "checkers".to_owned(),
            kind: Some(CommandLineOptionKind::Number),
            category: Some(command_line.to_owned()),
            description: Some("Set the number of checkers per project.".to_owned()),
            default_value_description: Some(DefaultValueDescription::String(
                "4, unless --singleThreaded is passed.".to_owned(),
            )),
            min_value: 1,
            ..CommandLineOption::default()
        },
    ]
}

pub fn options_for_build() -> Vec<CommandLineOption> {
    let command_line = "Command-line Options";

    vec![
        bool_option(
            "build",
            "b",
            command_line,
            "Build one or more projects and their dependencies, if out of date",
            true,
            false,
            false,
        ),
        bool_option(
            "verbose",
            "v",
            command_line,
            "Enable verbose logging.",
            false,
            false,
            false,
        ),
        bool_option(
            "dry",
            "d",
            command_line,
            "Show what would be built (or deleted, if specified with '--clean')",
            false,
            false,
            false,
        ),
        bool_option(
            "force",
            "f",
            command_line,
            "Build all projects, including those that appear to be up to date.",
            false,
            false,
            false,
        ),
        bool_option(
            "clean",
            "",
            command_line,
            "Delete the outputs of all projects.",
            false,
            false,
            false,
        ),
        CommandLineOption {
            name: "builders".to_owned(),
            kind: Some(CommandLineOptionKind::Number),
            category: Some(command_line.to_owned()),
            description: Some("Set the number of projects to build concurrently.".to_owned()),
            default_value_description: Some(DefaultValueDescription::String(
                "4, unless --singleThreaded is passed.".to_owned(),
            )),
            min_value: 1,
            ..CommandLineOption::default()
        },
        bool_option(
            "stopBuildOnErrors",
            "",
            command_line,
            "Skip building downstream projects on error in upstream project.",
            false,
            false,
            false,
        ),
    ]
}

pub fn build_opts() -> Vec<CommandLineOption> {
    let mut options = common_options_with_build();
    options.extend(options_for_build());
    options
}

use crate::CommandLineOption;
use crate::namemap::NameMap;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DidYouMeanOptionsDiagnostics {
    pub alternate_mode: Option<AlternateModeDiagnostics>,
    pub option_declarations: Vec<CommandLineOption>,
    pub unknown_option_diagnostic: String,
    pub unknown_did_you_mean_diagnostic: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AlternateModeDiagnostics {
    pub diagnostic: String,
    pub options_name_map: Option<NameMap>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ParseCommandLineWorkerDiagnostics {
    pub did_you_mean: DidYouMeanOptionsDiagnostics,
    pub option_type_mismatch_diagnostic: String,
}

pub fn get_parse_command_line_worker_diagnostics(
    decls: &[CommandLineOption],
    build_name_map: NameMap,
) -> ParseCommandLineWorkerDiagnostics {
    ParseCommandLineWorkerDiagnostics {
        did_you_mean: DidYouMeanOptionsDiagnostics {
            alternate_mode: Some(AlternateModeDiagnostics {
                diagnostic: "Compiler_option_0_may_only_be_used_with_build".to_owned(),
                options_name_map: Some(build_name_map),
            }),
            option_declarations: decls.to_vec(),
            unknown_option_diagnostic: "Unknown_compiler_option_0".to_owned(),
            unknown_did_you_mean_diagnostic: "Unknown_compiler_option_0_Did_you_mean_1".to_owned(),
        },
        option_type_mismatch_diagnostic: "Compiler_option_0_expects_an_argument".to_owned(),
    }
}

pub fn watch_options_did_you_mean_diagnostics(
    options_for_watch: &[CommandLineOption],
) -> ParseCommandLineWorkerDiagnostics {
    ParseCommandLineWorkerDiagnostics {
        did_you_mean: DidYouMeanOptionsDiagnostics {
            alternate_mode: None,
            option_declarations: options_for_watch.to_vec(),
            unknown_option_diagnostic: "Unknown_watch_option_0".to_owned(),
            unknown_did_you_mean_diagnostic: "Unknown_watch_option_0_Did_you_mean_1".to_owned(),
        },
        option_type_mismatch_diagnostic: "Watch_option_0_requires_a_value_of_type_1".to_owned(),
    }
}

pub fn build_options_did_you_mean_diagnostics(
    build_opts: &[CommandLineOption],
    compiler_name_map: NameMap,
) -> ParseCommandLineWorkerDiagnostics {
    ParseCommandLineWorkerDiagnostics {
        did_you_mean: DidYouMeanOptionsDiagnostics {
            alternate_mode: Some(AlternateModeDiagnostics {
                diagnostic: "Compiler_option_0_may_not_be_used_with_build".to_owned(),
                options_name_map: Some(compiler_name_map),
            }),
            option_declarations: build_opts.to_vec(),
            unknown_option_diagnostic: "Unknown_build_option_0".to_owned(),
            unknown_did_you_mean_diagnostic: "Unknown_build_option_0_Did_you_mean_1".to_owned(),
        },
        option_type_mismatch_diagnostic: "Build_option_0_requires_a_value_of_type_1".to_owned(),
    }
}

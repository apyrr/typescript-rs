use crate::tsc;

use ts_testutil::baseline;
use ts_tspath as tspath;
use ts_vfs::Fs;

use super::sys::{FileMap, TestSys, new_test_sys};

pub struct TscEdit {
    pub caption: String,
    pub command_line_args: Option<Vec<String>>,
    pub edit: Option<fn(&mut TestSys)>,
    pub expected_diff: String,
}

pub fn no_change() -> TscEdit {
    TscEdit {
        caption: "no change".to_owned(),
        command_line_args: None,
        edit: None,
        expected_diff: String::new(),
    }
}

pub fn no_change_only_edit() -> Vec<TscEdit> {
    vec![no_change()]
}

pub struct TscInput {
    pub sub_scenario: String,
    pub command_line_args: Vec<String>,
    pub files: FileMap,
    pub cwd: String,
    pub edits: Vec<TscEdit>,
    pub env: std::collections::HashMap<String, String>,
    pub ignore_case: bool,
    pub windows_style_root: String,
}

impl TscInput {
    pub fn execute_command(
        &self,
        sys: &mut TestSys,
        baseline_builder: &mut String,
        command_line_args: Vec<String>,
    ) -> tsc::CommandLineResult {
        baseline_builder.push_str("tsgo ");
        baseline_builder.push_str(&command_line_args.join(" "));
        baseline_builder.push('\n');
        let result = crate::command_line(
            sys.clone_system(),
            command_line_args,
            Some(sys.clone_testing()),
        );
        match result.status {
            tsc::ExitStatus::Success => baseline_builder.push_str("ExitStatus:: Success"),
            tsc::ExitStatus::DiagnosticsPresentOutputsSkipped => {
                baseline_builder.push_str("ExitStatus:: DiagnosticsPresent_OutputsSkipped")
            }
            tsc::ExitStatus::DiagnosticsPresentOutputsGenerated => {
                baseline_builder.push_str("ExitStatus:: DiagnosticsPresent_OutputsGenerated")
            }
            tsc::ExitStatus::InvalidProjectOutputsSkipped => {
                baseline_builder.push_str("ExitStatus:: InvalidProject_OutputsSkipped")
            }
            tsc::ExitStatus::ProjectReferenceCycleOutputsSkipped => {
                baseline_builder.push_str("ExitStatus:: ProjectReferenceCycle_OutputsSkipped")
            }
            tsc::ExitStatus::NotImplemented => {
                baseline_builder.push_str("ExitStatus:: NotImplemented")
            }
        }
        result
    }

    pub fn run(&self, t: &mut dyn TestingT, scenario: &str) {
        t.helper();
        t.run(
            &format!("{}/{}", self.get_baseline_sub_folder(), self.sub_scenario),
            &mut |t| {
                t.parallel();

                let mut baseline_builder = String::new();
                let mut sys = new_test_sys(self, false);
                baseline_builder.push_str("currentDirectory::");
                baseline_builder.push_str(&sys.get_current_directory());
                baseline_builder.push_str("\nuseCaseSensitiveFileNames::");
                baseline_builder.push_str(&sys.fs().use_case_sensitive_file_names().to_string());
                baseline_builder.push_str("\nInput::\n");
                sys.baseline_fs_with_diff(&mut baseline_builder);
                let mut result = self.execute_command(
                    &mut sys,
                    &mut baseline_builder,
                    self.command_line_args.clone(),
                );
                sys.serialize_state(&mut baseline_builder);
                let mut unexpected_diff = String::new();
                unexpected_diff
                    .push_str(&sys.baseline_programs(&mut baseline_builder, "Initial build"));

                for (index, do_edit) in self.edits.iter().enumerate() {
                    sys.clear_output();
                    let command_line_args = do_edit
                        .command_line_args
                        .clone()
                        .unwrap_or_else(|| self.command_line_args.clone());

                    baseline_builder.push_str(&format!(
                        "\n\nEdit [{index}]:: {}\n",
                        do_edit.caption
                    ));
                    if let Some(edit) = do_edit.edit {
                        edit(&mut sys);
                    }
                    sys.baseline_fs_with_diff(&mut baseline_builder);

                    if let Some(watcher) = result.watcher.as_mut() {
                        watcher.do_cycle();
                    } else {
                        self.execute_command(&mut sys, &mut baseline_builder, command_line_args.clone());
                    }
                    sys.serialize_state(&mut baseline_builder);
                    unexpected_diff.push_str(&sys.baseline_programs(
                        &mut baseline_builder,
                        &format!("Edit [{index}]:: {}\n", do_edit.caption),
                    ));

                    // PORT NOTE: reshaped for borrowck; Go runs this in the same
                    // WorkGroup as the edit command, but only observes both systems
                    // after RunAndWait completes.
                    let mut non_incremental_sys = new_test_sys(self, true);
                    for prior_edit in self.edits.iter().take(index + 1) {
                        if let Some(edit) = prior_edit.edit {
                            edit(&mut non_incremental_sys);
                        }
                    }
                    crate::command_line(
                        non_incremental_sys.clone_system(),
                        command_line_args,
                        Some(non_incremental_sys.clone_testing()),
                    );

                    let diff = get_diff_for_incremental(&sys, &non_incremental_sys);
                    if !diff.is_empty() {
                        let explanation = if do_edit.expected_diff.is_empty() {
                            "!!! Unexpected diff, please review and either fix or write explanation as expectedDiff !!!"
                        } else {
                            &do_edit.expected_diff
                        };
                        baseline_builder.push_str(&format!("\n\nDiff:: {explanation}\n"));
                        baseline_builder.push_str(&diff);
                        if do_edit.expected_diff.is_empty() {
                            unexpected_diff.push_str(&format!(
                                "Edit [{index}]:: {}\n!!! Unexpected diff, please review and either fix or write explanation as expectedDiff !!!\n{diff}\n",
                                do_edit.caption
                            ));
                        }
                    } else if !do_edit.expected_diff.is_empty() {
                        baseline_builder.push_str(&format!(
                            "\n\nDiff:: {} !!! Diff not found but explanation present, please review and remove the explanation !!!\n",
                            do_edit.expected_diff
                        ));
                        unexpected_diff.push_str(&format!(
                            "Edit [{index}]:: {}\n!!! Diff not found but explanation present, please review and remove the explanation !!!\n",
                            do_edit.caption
                        ));
                    }
                }

                let baseline_file_name = format!("{}.js", self.sub_scenario.replace(' ', "-"));
                if let Err(err) = baseline::run(
                    &baseline_file_name,
                    &baseline_builder,
                    baseline::Options {
                        subfolder: format!("{}/{}", self.get_baseline_sub_folder(), scenario),
                        ..Default::default()
                    },
                ) {
                    t.errorf(&err);
                }
                if !unexpected_diff.is_empty() {
                    t.errorf(&format!(
                        "Test {} has unexpected diff {} with incremental build, please review the baseline file",
                        self.sub_scenario, unexpected_diff
                    ));
                }
            },
        );
    }
}

pub fn get_diff_for_incremental(
    incremental_sys: &TestSys,
    non_incremental_sys: &TestSys,
) -> String {
    let mut diff_builder = String::new();

    let mut non_incremental_outputs = non_incremental_sys.fs().written_files.to_slice();
    non_incremental_outputs.sort();
    for non_incremental_output in non_incremental_outputs {
        if tspath::file_extension_is(&non_incremental_output, tspath::EXTENSION_TS_BUILD_INFO)
            || non_incremental_output.ends_with(".readable.baseline.txt")
        {
            if !incremental_sys
                .fs_from_file_map()
                .file_exists(&non_incremental_output)
            {
                diff_builder.push_str(&baseline::diff_text(
                    &format!("nonIncremental {non_incremental_output}"),
                    &format!("incremental {non_incremental_output}"),
                    "Exists",
                    "",
                ));
                diff_builder.push('\n');
            }
        } else {
            let (non_incremental_text, ok) = non_incremental_sys
                .fs_from_file_map()
                .read_file(&non_incremental_output);
            if !ok {
                panic!("Written file not found {non_incremental_output}");
            }
            let (incremental_text, ok) = incremental_sys
                .fs_from_file_map()
                .read_file(&non_incremental_output);
            if !ok || incremental_text != non_incremental_text {
                diff_builder.push_str(&baseline::diff_text(
                    &format!("nonIncremental {non_incremental_output}"),
                    &format!("incremental {non_incremental_output}"),
                    &non_incremental_text,
                    &incremental_text,
                ));
                diff_builder.push('\n');
            }
        }
    }

    let incremental_output = incremental_sys.get_output(true);
    let non_incremental_output = non_incremental_sys.get_output(true);
    if incremental_output != non_incremental_output {
        diff_builder.push_str(&baseline::diff_text(
            "nonIncremental.output.txt",
            "incremental.output.txt",
            &non_incremental_output,
            &incremental_output,
        ));
    }
    diff_builder
}

impl TscInput {
    pub fn get_baseline_sub_folder(&self) -> String {
        let mut command_name = "tsc";
        if self
            .command_line_args
            .iter()
            .any(|arg| matches!(arg.as_str(), "-b" | "--b" | "-build" | "--build"))
        {
            command_name = "tsbuild";
        }
        let mut w = "";
        if self
            .command_line_args
            .iter()
            .any(|arg| matches!(arg.as_str(), "-w" | "--w" | "-watch" | "--watch"))
        {
            w = "Watch";
        }
        format!("{command_name}{w}")
    }
}

pub trait TestingT {
    fn helper(&mut self);
    fn run(&mut self, name: &str, f: &mut dyn FnMut(&mut dyn TestingT));
    fn parallel(&mut self);
    fn errorf(&mut self, message: &str);
}

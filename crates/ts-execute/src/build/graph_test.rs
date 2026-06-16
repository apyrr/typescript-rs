use std::collections::HashMap;

use ts_core as core;
use ts_tsoptions as tsoptions;
use ts_tsoptions::tsoptionstest::VfsParseConfigHost;
use ts_vfs::vfstest::IntoMapFile;

use crate::tsctests;

use super as build;

#[test]
fn test_build_order_generator() {
    let test_cases = vec![
        BuildOrderTestCase {
            name: "specify two roots",
            projects: vec!["A", "G"],
            expected: vec!["D", "E", "C", "B", "A", "G"],
            circular: false,
        },
        BuildOrderTestCase {
            name: "multiple parts of the same graph in various orders",
            projects: vec!["A"],
            expected: vec!["D", "E", "C", "B", "A"],
            circular: false,
        },
        BuildOrderTestCase {
            name: "multiple parts of the same graph in various orders",
            projects: vec!["A", "C", "D"],
            expected: vec!["D", "E", "C", "B", "A"],
            circular: false,
        },
        BuildOrderTestCase {
            name: "multiple parts of the same graph in various orders",
            projects: vec!["D", "C", "A"],
            expected: vec!["D", "E", "C", "B", "A"],
            circular: false,
        },
        BuildOrderTestCase {
            name: "other orderings",
            projects: vec!["F"],
            expected: vec!["E", "F"],
            circular: false,
        },
        BuildOrderTestCase {
            name: "other orderings",
            projects: vec!["E"],
            expected: vec!["E"],
            circular: false,
        },
        BuildOrderTestCase {
            name: "other orderings",
            projects: vec!["F", "C", "A"],
            expected: vec!["E", "F", "D", "C", "B", "A"],
            circular: false,
        },
        BuildOrderTestCase {
            name: "returns circular order",
            projects: vec!["H"],
            expected: vec!["E", "J", "I", "H"],
            circular: true,
        },
        BuildOrderTestCase {
            name: "returns circular order",
            projects: vec!["A", "H"],
            expected: vec!["D", "E", "C", "B", "A", "J", "I", "H"],
            circular: true,
        },
    ];
    for testcase in &test_cases {
        testcase.run();
    }
}

struct BuildOrderTestCase {
    name: &'static str,
    projects: Vec<&'static str>,
    expected: Vec<&'static str>,
    circular: bool,
}

impl BuildOrderTestCase {
    fn config_name(&self, project: &str) -> String {
        format!("/home/src/workspaces/project/{project}/tsconfig.json")
    }

    fn project_name(&self, config: String) -> String {
        let str = config
            .strip_prefix("/home/src/workspaces/project/")
            .unwrap_or(&config);
        str.strip_suffix("/tsconfig.json").unwrap_or(str).to_owned()
    }

    fn parse_build_command_line(args: &[String]) -> tsoptions::ParsedBuildCommandLine {
        let host = VfsParseConfigHost::new(
            std::collections::BTreeMap::new(),
            "/home/src/workspaces/project",
            true,
        );
        let mut command = tsoptions::parse_build_command_line(args, host);
        if args.iter().any(|arg| arg == "--dry") {
            command.build_options.dry = core::Tristate::True;
        }
        if args.iter().any(|arg| arg == "--watch") {
            command.compiler_options.watch = core::Tristate::True;
        }
        command
    }

    fn run(&self) {
        let mut files: tsctests::FileMap = HashMap::new();
        let deps: HashMap<&str, Vec<&str>> = HashMap::from([
            ("A", vec!["B", "C"]),
            ("B", vec!["C", "D"]),
            ("C", vec!["D", "E"]),
            ("F", vec!["E"]),
            ("H", vec!["I"]),
            ("I", vec!["J"]),
            ("J", vec!["H", "E"]),
        ]);
        let mut reverse_deps: HashMap<&str, Vec<&str>> = HashMap::new();
        for (project, deps) in &deps {
            for dep in deps {
                reverse_deps.entry(dep).or_default().push(project);
            }
        }
        let verify_deps =
            |orchestrator: &build::Orchestrator, build_order: &[String], has_down_stream: bool| {
                for (index, project) in build_order.iter().enumerate() {
                    let upstream = core::map(
                        &orchestrator.upstream(&self.config_name(project)),
                        |config| self.project_name(config.clone()),
                    );
                    let expected_upstream = deps.get(project.as_str()).cloned().unwrap_or_default();
                    assert!(
                        upstream.len() <= expected_upstream.len(),
                        "Expected upstream for {} to be at most {}, got {}",
                        project,
                        expected_upstream.len(),
                        upstream.len()
                    );
                    for expected in &expected_upstream {
                        if build_order[..index]
                            .iter()
                            .any(|project| project == expected)
                        {
                            assert!(
                                upstream.iter().any(|project| project == expected),
                                "Expected upstream for {} to contain {}",
                                project,
                                expected
                            );
                        } else {
                            assert!(
                                !upstream.iter().any(|project| project == expected),
                                "Expected upstream for {} to not contain {}",
                                project,
                                expected
                            );
                        }
                    }

                    let downstream = core::map(
                        &orchestrator.downstream(&self.config_name(project)),
                        |config| self.project_name(config.clone()),
                    );
                    let expected_downstream = if has_down_stream {
                        reverse_deps
                            .get(project.as_str())
                            .cloned()
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    assert!(
                        downstream.len() <= expected_downstream.len(),
                        "Expected downstream for {} to be at most {}, got {}",
                        project,
                        expected_downstream.len(),
                        downstream.len()
                    );
                    for expected in &expected_downstream {
                        if build_order[index + 1..]
                            .iter()
                            .any(|project| project == expected)
                        {
                            assert!(
                                downstream.iter().any(|project| project == expected),
                                "Expected downstream for {} to contain {}",
                                project,
                                expected
                            );
                        } else {
                            assert!(
                                !downstream.iter().any(|project| project == expected),
                                "Expected downstream for {} to not contain {}",
                                project,
                                expected
                            );
                        }
                    }
                }
            };
        for project in ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"] {
            files.insert(
                format!("/home/src/workspaces/project/{project}/{project}.ts"),
                "export {}".into_map_file(std::time::SystemTime::UNIX_EPOCH),
            );
            let mut references_str = String::new();
            if let Some(deps) = deps.get(project) {
                references_str = format!(
                    r#", "references": [{}]"#,
                    core::map(deps, |dep| format!(r#"{{ "path": "../{dep}" }}"#)).join(",")
                );
            }
            files.insert(
                self.config_name(project),
                format!(
                    r#"{{
                "compilerOptions": {{ "composite": true }},
                "files": ["./{project}.ts"],
                {references_str}
            }}"#
                )
                .into_map_file(std::time::SystemTime::UNIX_EPOCH),
            );
        }

        let sys = tsctests::new_tsc_system(files, true, "/home/src/workspaces/project".to_owned());
        let mut args = vec!["--build".to_owned(), "--dry".to_owned()];
        args.extend(self.projects.iter().map(|project| project.to_string()));
        let build_command = Self::parse_build_command_line(&args);
        let mut orchestrator = build::new_orchestrator(build::Options {
            sys: sys.clone_system(),
            command: build_command,
            testing: Some(sys.clone_testing()),
        });
        orchestrator.generate_graph(None);
        let build_order = core::map(&orchestrator.order(), |config| {
            self.project_name(config.clone())
        });
        assert_eq!(build_order, self.expected, "{}", self.name);
        verify_deps(&orchestrator, &build_order, false);

        if !self.circular {
            for (project, project_deps) in &deps {
                let child = self.config_name(project);
                let child_index = build_order.iter().position(|config| config == &child);
                if child_index.is_none() {
                    continue;
                }
                let child_index = child_index.unwrap();
                for dep in project_deps {
                    let parent = self.config_name(dep);
                    let parent_index = build_order
                        .iter()
                        .position(|config| config == &parent)
                        .unwrap();

                    assert!(
                        child_index > parent_index,
                        "Expecting child {} to be built after parent {}",
                        project,
                        dep
                    );
                }
            }
        }

        orchestrator.generate_graph_reusing_old_tasks();
        let build_order2 = core::map(&orchestrator.order(), |config| {
            self.project_name(config.clone())
        });
        assert_eq!(build_order2, self.expected, "{}", self.name);

        let mut args_watch = vec!["--build".to_owned(), "--watch".to_owned()];
        args_watch.extend(self.projects.iter().map(|project| project.to_string()));
        let build_command_watch = Self::parse_build_command_line(&args_watch);
        orchestrator = build::new_orchestrator(build::Options {
            sys: sys.clone_system(),
            command: build_command_watch,
            testing: Some(sys.clone_testing()),
        });
        orchestrator.generate_graph(None);
        let build_order3 = core::map(&orchestrator.order(), |config| {
            self.project_name(config.clone())
        });
        verify_deps(&orchestrator, &build_order3, true);
    }
}

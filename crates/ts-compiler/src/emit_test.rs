use std::fmt::Write as _;

use ts_bundled as bundled;
use ts_core as core;
use ts_tsoptions as tsoptions;
use ts_vfs::vfstest;

use crate::{
    new_compiler_host, new_program, EmitOptions, ProgramLike, ProgramOptions, WriteFileData,
    EMIT_ALL,
};

// generateLongLineTS generates TypeScript source code that produces a single very long line.
// This simulates generated code (e.g., from code generators) that has no line breaks,
// which triggers O(n²) behavior in source map generation due to
// GetECMALineAndUTF16CharacterOfPosition scanning from line start for each position.
fn generate_long_line_ts(num_properties: usize) -> String {
    // Build a large object literal all on one line, with no line breaks.
    let mut b = String::new();
    b.push_str("export const data: Record<string, number> = {");
    for i in 0..num_properties {
        if i > 0 {
            b.push_str(", ");
        }
        write!(&mut b, "prop_{}: {}", i, i).unwrap();
    }
    b.push_str("};");
    b
}

fn parsed_command_line(
    file_names: Vec<String>,
    opts: core::CompilerOptions,
) -> tsoptions::ParsedCommandLine {
    let mut config = tsoptions::parsed_command_line_options(Default::default(), file_names);
    config.set_compiler_options(opts);
    config
}

fn program_options(
    current_directory: &str,
    fs: Box<dyn ts_vfs::Fs + Send + Sync>,
    file_names: Vec<String>,
    opts: core::CompilerOptions,
) -> ProgramOptions {
    ProgramOptions {
        config: Box::new(parsed_command_line(file_names, opts)),
        host: new_compiler_host(
            current_directory.to_string(),
            fs,
            bundled::lib_path(),
            None,
            None,
        ),
        use_source_of_project_reference: false,
        single_threaded: core::TS_UNKNOWN,
        create_checker_pool: None,
        typings_location: String::new(),
        project_name: String::new(),
        type_script_version: String::new(),
        tracing: None,
    }
}

// Discard written files — these benchmarks only care about emit performance.
fn nop_write_file(_file_name: &str, _text: &str, _data: &WriteFileData) -> Result<(), String> {
    Ok(())
}

fn emit_once(p: &impl ProgramLike) {
    p.emit(
        core::Context::background(),
        EmitOptions {
            target_source_file: None,
            emit_only: EMIT_ALL,
            write_file: Some(nop_write_file),
        },
    );
}

fn benchmark_emit_long_lines(iterations: usize) {
    if !bundled::EMBEDDED {
        return;
    }

    for num_props in [1000, 5000, 10000] {
        let source = generate_long_line_ts(num_props);

        let fs = vfstest::from_map(
            [("/dev/src/index.ts", source)],
            true, /*useCaseSensitiveFileNames*/
        );
        let fs = Box::new(bundled::wrap_fs(fs));

        let opts = core::CompilerOptions {
            target: core::ScriptTarget::ES2015,
            source_map: core::Tristate::True,
            out_dir: "/dev/out".to_string(),
            ..Default::default()
        };

        let p = new_program(program_options(
            "/dev/src",
            fs,
            vec!["/dev/src/index.ts".to_string()],
            opts,
        ));

        for _ in 0..iterations {
            emit_once(&p);
        }
    }
}

fn benchmark_emit_many_files(iterations: usize) {
    if !bundled::EMBEDDED {
        return;
    }

    // Simulate many files with moderately long single-line content.
    let num_files = 200;
    let num_props_per_file = 500;

    let mut files = Vec::with_capacity(num_files);
    let mut file_names = Vec::with_capacity(num_files);
    for i in 0..num_files {
        let name = format!("/dev/src/file_{}.ts", i);
        files.push((name.clone(), generate_long_line_ts(num_props_per_file)));
        file_names.push(name);
    }

    let fs = vfstest::from_map(files, true);
    let fs = Box::new(bundled::wrap_fs(fs));

    let opts = core::CompilerOptions {
        target: core::ScriptTarget::ES2015,
        source_map: core::Tristate::True,
        out_dir: "/dev/out".to_string(),
        ..Default::default()
    };

    let p = new_program(program_options("/dev/src", fs, file_names, opts));

    for _ in 0..iterations {
        emit_once(&p);
    }
}

// BenchmarkEmitLongLinesWithLineBreaks is a control benchmark that emits the same amount
// of code but WITH line breaks, showing that the issue is specific to long lines.
fn benchmark_emit_long_lines_with_line_breaks(iterations: usize) {
    if !bundled::EMBEDDED {
        return;
    }

    let num_properties = 10000;

    // Same content but with newlines between each property.
    let mut sb = String::new();
    sb.push_str("export const data: Record<string, number> = {\n");
    for i in 0..num_properties {
        if i > 0 {
            sb.push_str(",\n");
        }
        write!(&mut sb, "  prop_{}: {}", i, i).unwrap();
    }
    sb.push_str("\n};\n");
    let source = sb;

    let fs = vfstest::from_map([("/dev/src/index.ts", source)], true);
    let fs = Box::new(bundled::wrap_fs(fs));

    let opts = core::CompilerOptions {
        target: core::ScriptTarget::ES2015,
        source_map: core::Tristate::True,
        out_dir: "/dev/out".to_string(),
        ..Default::default()
    };

    let p = new_program(program_options(
        "/dev/src",
        fs,
        vec!["/dev/src/index.ts".to_string()],
        opts,
    ));

    for _ in 0..iterations {
        emit_once(&p);
    }
}

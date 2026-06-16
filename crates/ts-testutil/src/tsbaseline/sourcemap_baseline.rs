use crate::baseline;
use crate::harnessutil::{CompilerOptions, Diagnostic, HarnessOptions, TestFile};
use serde::Deserialize;
use ts_collections::OrderedMap;

#[derive(Deserialize)]
struct RawSourceMapForBaseline {
    #[serde(default)]
    file: String,
    #[serde(default)]
    sources: Vec<String>,
}

pub struct SourceMapBaselineInput<'a> {
    pub baseline_path: &'a str,
    pub options: &'a CompilerOptions,
    pub diagnostics: &'a [Diagnostic],
    pub maps: &'a OrderedMap<String, String>,
    pub js: &'a OrderedMap<String, String>,
    pub outputs: &'a [TestFile],
    pub inputs: &'a [TestFile],
    pub harness_settings: &'a HarnessOptions,
    pub opts: baseline::Options,
}

pub fn do_sourcemap_baseline(input: SourceMapBaselineInput<'_>) -> Result<(), String> {
    let SourceMapBaselineInput {
        baseline_path,
        options,
        diagnostics,
        maps,
        js,
        outputs,
        inputs,
        harness_settings,
        opts,
    } = input;
    let declaration_maps = options.declaration_map && (options.declaration || options.composite);
    if options.inline_source_map {
        if !maps.is_empty() && !declaration_maps {
            return Err(
                "No sourcemap files should be generated if inlineSourceMaps was set.".into(),
            );
        }
        return Ok(());
    }
    if !(options.source_map || declaration_maps) {
        return Ok(());
    }

    let mut expected_map_count = 0;
    if options.source_map {
        expected_map_count += super::get_number_of_js_files(js, false);
    }
    if declaration_maps {
        expected_map_count += super::get_number_of_js_files(js, true);
    }
    if maps.size() != expected_map_count {
        return Err("Number of sourcemap files should be same as js files.".into());
    }

    let source_map_code = if options.no_emit_on_error && !diagnostics.is_empty() || maps.is_empty()
    {
        baseline::NO_CONTENT.to_string()
    } else {
        let mut source_map_code = String::new();
        for (unit_name, content) in maps.entries() {
            if !source_map_code.is_empty() {
                source_map_code.push_str("\r\n");
            }
            source_map_code.push_str(&super::file_output(
                &TestFile {
                    unit_name: unit_name.clone(),
                    content: content.clone(),
                },
                harness_settings,
            ));
            source_map_code.push_str(&create_source_map_preview_link(
                &TestFile {
                    unit_name: unit_name.clone(),
                    content: content.clone(),
                },
                outputs,
                inputs,
            ));
        }
        source_map_code
    };

    let baseline_path = if baseline_path.ends_with(".ts") || baseline_path.ends_with(".tsx") {
        format!(
            "{}.js.map",
            baseline_path
                .rsplit_once('.')
                .map(|(base, _)| base)
                .unwrap_or(baseline_path)
        )
    } else {
        baseline_path.to_string()
    };
    baseline::run(&baseline_path, &source_map_code, opts)
}

pub fn create_source_map_preview_link(
    source_map: &TestFile,
    outputs: &[TestFile],
    inputs: &[TestFile],
) -> String {
    let sourcemap_json: RawSourceMapForBaseline =
        serde_json::from_str(&source_map.content).unwrap_or_else(|err| panic!("{err}"));
    let Some(output_js_file) = outputs
        .iter()
        .find(|output| output.unit_name.ends_with(&sourcemap_json.file))
    else {
        return String::new();
    };
    let mut source_files = Vec::with_capacity(sourcemap_json.sources.len());
    for source_name in sourcemap_json.sources {
        let Some(source_file) = inputs
            .iter()
            .find(|input| input.unit_name.ends_with(&source_name))
        else {
            return String::new();
        };
        source_files.push(source_file);
    }

    let mut hash = String::from("\n//// https://sokra.github.io/source-map-visualization#base64,");
    hash.push_str(&base64_encode_chunk(&output_js_file.content));
    hash.push(',');
    hash.push_str(&base64_encode_chunk(&source_map.content));
    for input in source_files {
        hash.push(',');
        hash.push_str(&base64_encode_chunk(&input.content));
    }
    hash.push('\n');
    hash
}

pub fn base64_encode_chunk(s: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = s.as_bytes();
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0b11) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0b1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0b11_1111) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

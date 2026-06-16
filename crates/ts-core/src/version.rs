use std::sync::OnceLock;

// This is a var so it can be overridden by ldflags.
static VERSION: &str = match option_env!("TSGO_VERSION") {
    Some(version) => version,
    None => "7.0.0-dev",
};
static VERSION_MAJOR_MINOR: OnceLock<String> = OnceLock::new();

pub fn version() -> &'static str {
    VERSION
}

pub fn version_major_minor() -> String {
    VERSION_MAJOR_MINOR
        .get_or_init(|| {
            let mut seen_major = false;
            let i = VERSION
                .char_indices()
                .find_map(|(i, r)| {
                    if r == '.' {
                        if seen_major {
                            return Some(i);
                        }
                        seen_major = true;
                    }
                    None
                })
                .unwrap_or_else(|| panic!("invalid version string: {VERSION}"));
            VERSION[..i].to_string()
        })
        .clone()
}

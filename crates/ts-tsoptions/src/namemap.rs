use std::collections::BTreeMap;

use crate::CommandLineOption;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NameMap {
    options_names: BTreeMap<String, CommandLineOption>,
    short_option_names: BTreeMap<String, String>,
}

impl NameMap {
    pub fn get(&self, name: &str) -> Option<&CommandLineOption> {
        self.options_names.get(&name.to_lowercase())
    }

    pub fn get_from_short(&self, short_name: &str) -> Option<&CommandLineOption> {
        self.short_option_names
            .get(short_name)
            .and_then(|name| self.get(name))
    }

    pub fn get_option_declaration_from_name(
        &self,
        option_name: &str,
        allow_short: bool,
    ) -> Option<&CommandLineOption> {
        let mut option_name = option_name.to_lowercase();
        if allow_short && let Some(short) = self.short_option_names.get(&option_name) {
            option_name = short.clone();
        }
        self.get(&option_name)
    }
}

pub fn get_name_map_from_list(opt_decls: &[CommandLineOption]) -> NameMap {
    let mut options_names = BTreeMap::new();
    let mut short_option_names = BTreeMap::new();
    for option in opt_decls {
        options_names.insert(option.name.to_lowercase(), option.clone());
        if !option.short_name.is_empty() {
            short_option_names.insert(option.short_name.clone(), option.name.clone());
        }
    }
    NameMap {
        options_names,
        short_option_names,
    }
}

pub fn compiler_name_map(options_declarations: &[CommandLineOption]) -> NameMap {
    get_name_map_from_list(options_declarations)
}

pub fn build_name_map(build_opts: &[CommandLineOption]) -> NameMap {
    get_name_map_from_list(build_opts)
}

pub fn watch_name_map(options_for_watch: &[CommandLineOption]) -> NameMap {
    get_name_map_from_list(options_for_watch)
}

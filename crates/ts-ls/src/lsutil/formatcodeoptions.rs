use ts_core::{Tristate, bool_to_tristate};
use ts_lsproto::FormattingOptions;
use ts_printer::get_default_indent_size;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub enum IndentStyle {
    None,
    Block,
    Smart,
}

pub enum FormatSettingValue<'a> {
    String(&'a str),
    Float(f64),
    Int(i32),
}

pub fn parse_indent_style(v: FormatSettingValue<'_>) -> IndentStyle {
    match v {
        FormatSettingValue::String(s) => match s.to_ascii_lowercase().as_str() {
            "none" => IndentStyle::None,
            "block" => IndentStyle::Block,
            "smart" => IndentStyle::Smart,
            _ => IndentStyle::Smart,
        },
        FormatSettingValue::Float(s) => indent_style_from_i32(s as i32),
        FormatSettingValue::Int(s) => indent_style_from_i32(s),
    }
}

fn indent_style_from_i32(v: i32) -> IndentStyle {
    match v {
        0 => IndentStyle::None,
        1 => IndentStyle::Block,
        2 => IndentStyle::Smart,
        _ => IndentStyle::Smart,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub enum SemicolonPreference {
    Ignore,
    Insert,
    Remove,
}

pub fn parse_semicolon_preference(v: FormatSettingValue<'_>) -> SemicolonPreference {
    if let FormatSettingValue::String(s) = v {
        match s.to_ascii_lowercase().as_str() {
            "ignore" => return SemicolonPreference::Ignore,
            "insert" => return SemicolonPreference::Insert,
            "remove" => return SemicolonPreference::Remove,
            _ => {}
        }
    }
    SemicolonPreference::Ignore
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct EditorSettings {
    pub base_indent_size: i32,
    pub indent_size: i32,
    pub tab_size: i32,
    pub new_line_character: String,
    pub convert_tabs_to_spaces: Tristate,
    pub indent_style: IndentStyle,
    pub trim_trailing_whitespace: Tristate,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct FormatCodeSettings {
    pub editor_settings: EditorSettings,
    pub insert_space_after_comma_delimiter: Tristate,
    pub insert_space_after_semicolon_in_for_statements: Tristate,
    pub insert_space_before_and_after_binary_operators: Tristate,
    pub insert_space_after_constructor: Tristate,
    pub insert_space_after_keywords_in_control_flow_statements: Tristate,
    pub insert_space_after_function_keyword_for_anonymous_functions: Tristate,
    pub insert_space_after_opening_and_before_closing_nonempty_parenthesis: Tristate,
    pub insert_space_after_opening_and_before_closing_nonempty_brackets: Tristate,
    pub insert_space_after_opening_and_before_closing_nonempty_braces: Tristate,
    pub insert_space_after_opening_and_before_closing_empty_braces: Tristate,
    pub insert_space_after_opening_and_before_closing_template_string_braces: Tristate,
    pub insert_space_after_opening_and_before_closing_jsx_expression_braces: Tristate,
    pub insert_space_after_type_assertion: Tristate,
    pub insert_space_before_function_parenthesis: Tristate,
    pub place_open_brace_on_new_line_for_functions: Tristate,
    pub place_open_brace_on_new_line_for_control_blocks: Tristate,
    pub insert_space_before_type_annotation: Tristate,
    pub indent_multi_line_object_literal_beginning_on_blank_line: Tristate,
    pub semicolons: SemicolonPreference,
    pub indent_switch_case: Tristate,
}

impl Default for FormatCodeSettings {
    fn default() -> Self {
        get_default_format_code_settings()
    }
}

pub fn from_ls_format_options(
    f: FormatCodeSettings,
    opt: &FormattingOptions,
) -> FormatCodeSettings {
    let mut updated_settings = f;
    updated_settings.editor_settings.tab_size = opt.tab_size as i32;
    updated_settings.editor_settings.indent_size = opt.tab_size as i32;
    updated_settings.editor_settings.convert_tabs_to_spaces = bool_to_tristate(opt.insert_spaces);
    if let Some(trim_trailing_whitespace) = opt.trim_trailing_whitespace {
        updated_settings.editor_settings.trim_trailing_whitespace =
            bool_to_tristate(trim_trailing_whitespace);
    }
    updated_settings
}

impl FormatCodeSettings {
    pub fn to_ls_format_options(&self) -> FormattingOptions {
        FormattingOptions {
            tab_size: self.editor_settings.tab_size as u32,
            insert_spaces: self.editor_settings.convert_tabs_to_spaces.is_true(),
            trim_trailing_whitespace: Some(self.editor_settings.trim_trailing_whitespace.is_true()),
            insert_final_newline: None,
            trim_final_newlines: None,
        }
    }
}

pub fn get_default_format_code_settings() -> FormatCodeSettings {
    FormatCodeSettings {
        editor_settings: EditorSettings {
            base_indent_size: 0,
            indent_size: get_default_indent_size(),
            tab_size: get_default_indent_size(),
            new_line_character: "\n".to_string(),
            convert_tabs_to_spaces: Tristate::True,
            indent_style: IndentStyle::Smart,
            trim_trailing_whitespace: Tristate::True,
        },
        insert_space_after_constructor: Tristate::False,
        insert_space_after_comma_delimiter: Tristate::True,
        insert_space_after_semicolon_in_for_statements: Tristate::True,
        insert_space_before_and_after_binary_operators: Tristate::True,
        insert_space_after_keywords_in_control_flow_statements: Tristate::True,
        insert_space_after_function_keyword_for_anonymous_functions: Tristate::False,
        insert_space_after_opening_and_before_closing_nonempty_parenthesis: Tristate::False,
        insert_space_after_opening_and_before_closing_nonempty_brackets: Tristate::False,
        insert_space_after_opening_and_before_closing_nonempty_braces: Tristate::True,
        insert_space_after_opening_and_before_closing_empty_braces: Tristate::Unknown,
        insert_space_after_opening_and_before_closing_template_string_braces: Tristate::False,
        insert_space_after_opening_and_before_closing_jsx_expression_braces: Tristate::False,
        insert_space_after_type_assertion: Tristate::Unknown,
        insert_space_before_function_parenthesis: Tristate::False,
        place_open_brace_on_new_line_for_functions: Tristate::False,
        place_open_brace_on_new_line_for_control_blocks: Tristate::False,
        insert_space_before_type_annotation: Tristate::Unknown,
        indent_multi_line_object_literal_beginning_on_blank_line: Tristate::Unknown,
        semicolons: SemicolonPreference::Ignore,
        indent_switch_case: Tristate::True,
    }
}

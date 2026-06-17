use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_locale as locale;
use ts_lsproto as lsproto;

use crate::codeactions::{CodeAction, CodeFixContext, CodeFixProvider};

pub fn spelling_error_codes() -> Vec<i32> {
    vec![
        diagnostics::Property_0_does_not_exist_on_type_1_Did_you_mean_2.code(),
        diagnostics::Property_0_may_not_exist_on_type_1_Did_you_mean_2.code(),
    ]
}

pub static SPELLING_PROVIDER: CodeFixProvider = CodeFixProvider {
    error_codes: spelling_error_codes,
    get_code_actions: get_spelling_code_actions,
    fix_ids: &[],
    get_all_code_actions: None,
};

pub fn get_spelling_code_actions(
    _context: &core::Context,
    fix_context: &CodeFixContext,
) -> Result<Vec<CodeAction>, core::Error> {
    let Some(diagnostic) = fix_context.diagnostic else {
        return Ok(Vec::new());
    };
    let Some(suggestion) = spelling_suggestion_from_message(&diagnostic.message) else {
        return Ok(Vec::new());
    };

    Ok(vec![CodeAction {
        description: diagnostics::Change_spelling_to_0
            .localize(locale::und(), vec![Box::new(suggestion.clone())]),
        changes: vec![lsproto::TextEdit {
            range: diagnostic.range,
            new_text: suggestion,
        }],
        fix_id: String::new(),
        fix_all_description: String::new(),
    }])
}

fn spelling_suggestion_from_message(message: &str) -> Option<String> {
    let did_you_mean = message.find("Did you mean")?;
    let tail = &message[did_you_mean..];
    let start = tail.find('\'')?;
    let rest = &tail[start + 1..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_owned())
}

#[cfg(test)]
mod tests {
    use super::spelling_suggestion_from_message;

    #[test]
    fn spelling_suggestion_from_message_extracts_property_suggestion() {
        let message =
            "Property 'toStrang' does not exist on type 'string'. Did you mean 'toString'?";

        assert_eq!(
            spelling_suggestion_from_message(message),
            Some("toString".to_owned())
        );
    }
}

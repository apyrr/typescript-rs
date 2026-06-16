use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use regex::Regex;
use ts_core as core;
use ts_locale as locale;

use crate::category::{Key, Message};

pub trait DiagnosticArg: std::any::Any + std::fmt::Display {
    fn as_any(&self) -> &dyn std::any::Any;
}

impl<T> DiagnosticArg for T
where
    T: std::any::Any + std::fmt::Display,
{
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub type Any = Box<dyn DiagnosticArg>;
pub type Argument = Any;

impl Clone for Any {
    fn clone(&self) -> Self {
        Box::new(self.to_string())
    }
}

impl From<String> for Any {
    fn from(value: String) -> Self {
        Box::new(value)
    }
}

impl From<&String> for Any {
    fn from(value: &String) -> Self {
        Box::new(value.clone())
    }
}

impl From<&'static str> for Any {
    fn from(value: &'static str) -> Self {
        Box::new(value)
    }
}

impl From<i32> for Any {
    fn from(value: i32) -> Self {
        Box::new(value)
    }
}

impl From<usize> for Any {
    fn from(value: usize) -> Self {
        Box::new(value)
    }
}

// Package diagnostics contains generated localizable diagnostic messages.

//go:generate go run generate.go -diagnostics ./diagnostics_generated.go -loc ./loc_generated.go -locdir ./loc
//go:generate go tool golang.org/x/tools/cmd/stringer -type=Category -output=stringer_generated.go
//go:generate npx dprint fmt diagnostics_generated.go loc_generated.go stringer_generated.go

impl Message {
    // For debugging only.
    pub fn string(&self) -> String {
        self.text.clone()
    }

    pub fn localize(&self, locale: locale::Locale, args: Vec<Any>) -> String {
        localize(locale, Some(self), String::new(), stringify_args(args))
    }
}

pub fn localize<I, S>(
    locale: locale::Locale,
    message: Option<&Message>,
    key: Key,
    args: I,
) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let message = if let Some(message) = message {
        message.clone()
    } else {
        key_to_message(&key)
            .unwrap_or_else(|| panic!("Unknown diagnostic message: {key}"))
            .clone()
    };

    let mut text = message.text.clone();
    if let Some(localized) = get_localized_messages(locale).get(&message.key) {
        text = localized.clone();
    }

    format(
        &text,
        args.into_iter()
            .map(|arg| arg.as_ref().to_string())
            .collect(),
    )
}

fn localized_messages_cache() -> &'static Mutex<HashMap<locale::Locale, HashMap<Key, String>>> {
    static CACHE: OnceLock<Mutex<HashMap<locale::Locale, HashMap<Key, String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

type LocalizedMessagesCacheGuard =
    std::sync::MutexGuard<'static, HashMap<locale::Locale, HashMap<Key, String>>>;

fn lock_localized_messages_cache() -> LocalizedMessagesCacheGuard {
    localized_messages_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn get_localized_messages(loc: locale::Locale) -> HashMap<Key, String> {
    if loc.is_und() {
        return HashMap::new();
    }

    // Check cache first
    if let Some(cached) = lock_localized_messages_cache().get(&loc).cloned() {
        return cached;
    }

    let messages = if let Some(index) = matcher_match(loc.clone()) {
        locale_funcs()
            .get(index)
            .and_then(|func| func.as_ref())
            .map(|func| func())
            .unwrap_or_default()
    } else {
        HashMap::new()
    };

    lock_localized_messages_cache().insert(loc, messages.clone());
    messages
}

fn placeholder_regexp() -> &'static Regex {
    static PLACEHOLDER_REGEXP: OnceLock<Regex> = OnceLock::new();
    PLACEHOLDER_REGEXP.get_or_init(|| Regex::new(r"\{(\d+)\}").unwrap())
}

pub fn format(text: &str, mut args: Vec<String>) -> String {
    if args.is_empty() {
        return text.to_string();
    }

    // Replace invalid UTF-8 with Unicode replacement character
    args = core::same_map(&args, |arg| {
        String::from_utf8_lossy(arg.as_bytes()).to_string()
    });

    placeholder_regexp()
        .replace_all(text, |captures: &regex::Captures| {
            let index = captures[1].parse::<usize>().unwrap_or(usize::MAX);
            if index >= args.len() {
                panic!("Invalid formatting placeholder");
            }
            args[index].clone()
        })
        .to_string()
}

pub fn stringify_args(args: Vec<Any>) -> Vec<String> {
    if args.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(args.len());
    for arg in args {
        result.push(arg.to_string());
    }
    result
}

fn key_to_message(key: &Key) -> Option<&'static Message> {
    crate::diagnostics_generated::key_to_message_generated(key)
}

fn matcher_match(_loc: locale::Locale) -> Option<usize> {
    crate::loc_generated::matcher_match_generated(_loc)
}

type LocaleFunc = fn() -> HashMap<Key, String>;

fn locale_funcs() -> Vec<Option<LocaleFunc>> {
    crate::loc_generated::locale_funcs_generated()
}

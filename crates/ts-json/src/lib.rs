#![forbid(unsafe_code)]
use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

//nolint:depguard

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Options {
    AllowDuplicateNames(bool),
    Deterministic(bool),
    WithIndent(String),
    WithIndentPrefix(String),
    AllowInvalidUtf8(bool),
}

pub type Value = serde_json::Value;
pub type Kind = &'static str;
pub type Decoder<R> = serde_json::Deserializer<serde_json::de::IoRead<R>>;
pub type Encoder<W> = serde_json::Serializer<W>;

pub const BEGIN_OBJECT: Kind = "begin_object";
pub const END_OBJECT: Kind = "end_object";
pub const NULL: Kind = "null";
pub const BEGIN_ARRAY: Kind = "begin_array";
pub const END_ARRAY: Kind = "end_array";

fn allow_invalid() -> Vec<Options> {
    vec![Options::AllowInvalidUtf8(true)]
}

fn with_allow_invalid(opts: &[Options]) -> Vec<Options> {
    if opts.is_empty() {
        allow_invalid()
    } else {
        let mut result = allow_invalid();
        result.extend_from_slice(opts);
        result
    }
}

fn indent_from_options(opts: &[Options]) -> Option<String> {
    opts.iter().rev().find_map(|opt| match opt {
        Options::WithIndent(indent) => Some(indent.clone()),
        _ => None,
    })
}

pub fn marshal<T: Serialize>(input: &T, opts: &[Options]) -> serde_json::Result<Vec<u8>> {
    let opts = with_allow_invalid(opts);
    if indent_from_options(&opts).is_some() {
        serde_json::to_vec_pretty(input)
    } else {
        serde_json::to_vec(input)
    }
}

pub fn marshal_encode<W: Write, T: Serialize>(
    out: &mut Encoder<W>,
    input: &T,
    opts: &[Options],
) -> serde_json::Result<()> {
    let opts = with_allow_invalid(opts);
    if indent_from_options(&opts).is_some() {
        // PORT NOTE: serde_json::Serializer does not let callers switch an existing
        // serializer into pretty mode, so the option is observed by marshal and
        // marshal_write but not by this already-constructed encoder path.
    }
    input.serialize(out)
}

pub fn marshal_write<W: Write, T: Serialize>(
    out: &mut W,
    input: &T,
    opts: &[Options],
) -> serde_json::Result<()> {
    let opts = with_allow_invalid(opts);
    if let Some(indent) = indent_from_options(&opts) {
        let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
        let mut serializer = serde_json::Serializer::with_formatter(out, formatter);
        input.serialize(&mut serializer)
    } else {
        serde_json::to_writer(out, input)
    }
}

pub fn marshal_indent<T: Serialize>(
    input: &T,
    prefix: &str,
    indent: &str,
) -> serde_json::Result<Vec<u8>> {
    if prefix.is_empty() && indent.is_empty() {
        // WithIndentPrefix and WithIndent imply multiline output, so skip them.
        return marshal(input, &[]);
    }
    marshal(
        input,
        &[
            Options::WithIndentPrefix(prefix.to_owned()),
            Options::WithIndent(indent.to_owned()),
        ],
    )
}

pub fn marshal_indent_write<W: Write, T: Serialize>(
    out: &mut W,
    input: &T,
    prefix: &str,
    indent: &str,
) -> serde_json::Result<()> {
    if prefix.is_empty() && indent.is_empty() {
        // WithIndentPrefix and WithIndent imply multiline output, so skip them.
        return marshal_write(out, input, &[]);
    }
    marshal_write(
        out,
        input,
        &[
            Options::WithIndentPrefix(prefix.to_owned()),
            Options::WithIndent(indent.to_owned()),
        ],
    )
}

pub fn unmarshal<T: DeserializeOwned>(
    input: &[u8],
    out: &mut T,
    _opts: &[Options],
) -> serde_json::Result<()> {
    *out = serde_json::from_slice(input)?;
    Ok(())
}

pub fn unmarshal_decode<'de, R, T>(
    input: &mut serde_json::Deserializer<R>,
    out: &mut T,
    _opts: &[Options],
) -> serde_json::Result<()>
where
    R: serde_json::de::Read<'de>,
    T: Deserialize<'de>,
{
    *out = T::deserialize(input)?;
    Ok(())
}

pub fn unmarshal_read<R: Read, T: DeserializeOwned>(
    input: R,
    out: &mut T,
    _opts: &[Options],
) -> serde_json::Result<()> {
    *out = serde_json::from_reader(input)?;
    Ok(())
}

pub fn allow_duplicate_names(allow: bool) -> Options {
    Options::AllowDuplicateNames(allow)
}

pub fn deterministic(value: bool) -> Options {
    Options::Deterministic(value)
}

pub fn with_indent(indent: &str) -> Options {
    Options::WithIndent(indent.to_owned())
}

pub fn new_decoder<R: Read>(reader: R) -> Decoder<R> {
    serde_json::Deserializer::from_reader(reader)
}

pub trait UnmarshalerFrom {}

pub trait MarshalerTo {}

pub fn write_all_json<W: Write, T: Serialize>(
    out: &mut W,
    input: &T,
    opts: &[Options],
) -> io::Result<()> {
    marshal_write(out, input, opts).map_err(io::Error::other)
}

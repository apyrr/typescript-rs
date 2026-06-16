use crate::CompletionsExpectedItem;
use ts_lsproto as lsproto;

pub struct Ignored;

pub fn ignored() -> Ignored {
    Ignored
}

pub fn default_commit_characters() -> Vec<String> {
    [".", ",", ";"]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

const SORT_TEXT_GLOBALS_OR_KEYWORDS: &str = "15";
const SORT_TEXT_LOCATION_PRIORITY: &str = "11";

fn deprecate_sort_text(original: &str) -> String {
    format!("z{original}")
}

fn item(
    label: &str,
    kind: lsproto::CompletionItemKind,
    sort_text: Option<String>,
    deprecated: bool,
) -> CompletionsExpectedItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = label.to_string();
    item.kind = Some(kind);
    item.sort_text = sort_text;
    if deprecated {
        item.tags = Some(vec![lsproto::CompletionItemTag::DEPRECATED]);
    }
    CompletionsExpectedItem::Item(item)
}

fn spec_item(line: &str, sort_text: Option<&str>) -> CompletionsExpectedItem {
    let mut parts = line.split('|');
    let label = parts.next().unwrap();
    let kind = match parts.next().unwrap() {
        "Class" => lsproto::CompletionItemKind::CLASS,
        "Field" => lsproto::CompletionItemKind::FIELD,
        "Function" => lsproto::CompletionItemKind::FUNCTION,
        "Interface" => lsproto::CompletionItemKind::INTERFACE,
        "Keyword" => lsproto::CompletionItemKind::KEYWORD,
        "Method" => lsproto::CompletionItemKind::METHOD,
        "Module" => lsproto::CompletionItemKind::MODULE,
        "Variable" => lsproto::CompletionItemKind::VARIABLE,
        kind => panic!("unexpected completion item kind: {kind}"),
    };
    let deprecated = parts.next().is_some_and(|value| value == "1");
    let sort_text = sort_text.map(|value| {
        if deprecated {
            deprecate_sort_text(value)
        } else {
            value.to_string()
        }
    });
    item(label, kind, sort_text, deprecated)
}

fn items_from_specs(specs: &str, sort_text: Option<&str>) -> Vec<CompletionsExpectedItem> {
    specs
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| spec_item(line, sort_text))
        .collect()
}

pub fn completion_global_this_item() -> CompletionsExpectedItem {
    item(
        "globalThis",
        lsproto::CompletionItemKind::MODULE,
        Some(SORT_TEXT_GLOBALS_OR_KEYWORDS.to_string()),
        false,
    )
}

pub fn completion_undefined_var_item() -> CompletionsExpectedItem {
    item(
        "undefined",
        lsproto::CompletionItemKind::VARIABLE,
        Some(SORT_TEXT_GLOBALS_OR_KEYWORDS.to_string()),
        false,
    )
}

pub fn completion_global_vars() -> Vec<CompletionsExpectedItem> {
    items_from_specs(
        r#"Array|Variable|0
ArrayBuffer|Variable|0
Boolean|Variable|0
DataView|Variable|0
Date|Variable|0
Error|Variable|0
EvalError|Variable|0
Float32Array|Variable|0
Float64Array|Variable|0
Function|Variable|0
Infinity|Variable|0
Int16Array|Variable|0
Int32Array|Variable|0
Int8Array|Variable|0
Intl|Module|0
JSON|Variable|0
Math|Variable|0
NaN|Variable|0
Number|Variable|0
Object|Variable|0
RangeError|Variable|0
ReferenceError|Variable|0
RegExp|Variable|0
String|Variable|0
SyntaxError|Variable|0
TypeError|Variable|0
URIError|Variable|0
Uint16Array|Variable|0
Uint32Array|Variable|0
Uint8Array|Variable|0
Uint8ClampedArray|Variable|0
decodeURI|Function|0
decodeURIComponent|Function|0
encodeURI|Function|0
encodeURIComponent|Function|0
eval|Function|0
isFinite|Function|0
isNaN|Function|0
parseFloat|Function|0
parseInt|Function|0
escape|Function|1
unescape|Function|1"#,
        Some(SORT_TEXT_GLOBALS_OR_KEYWORDS),
    )
}

pub fn completion_global_keywords() -> Vec<CompletionsExpectedItem> {
    items_from_specs(
        r#"abstract|Keyword|0
any|Keyword|0
as|Keyword|0
asserts|Keyword|0
async|Keyword|0
await|Keyword|0
bigint|Keyword|0
boolean|Keyword|0
break|Keyword|0
case|Keyword|0
catch|Keyword|0
class|Keyword|0
const|Keyword|0
continue|Keyword|0
debugger|Keyword|0
declare|Keyword|0
default|Keyword|0
delete|Keyword|0
do|Keyword|0
else|Keyword|0
enum|Keyword|0
export|Keyword|0
extends|Keyword|0
false|Keyword|0
finally|Keyword|0
for|Keyword|0
function|Keyword|0
if|Keyword|0
implements|Keyword|0
import|Keyword|0
in|Keyword|0
infer|Keyword|0
instanceof|Keyword|0
interface|Keyword|0
keyof|Keyword|0
let|Keyword|0
module|Keyword|0
namespace|Keyword|0
never|Keyword|0
new|Keyword|0
null|Keyword|0
number|Keyword|0
object|Keyword|0
package|Keyword|0
readonly|Keyword|0
return|Keyword|0
satisfies|Keyword|0
string|Keyword|0
super|Keyword|0
switch|Keyword|0
symbol|Keyword|0
this|Keyword|0
throw|Keyword|0
true|Keyword|0
try|Keyword|0
type|Keyword|0
typeof|Keyword|0
unique|Keyword|0
unknown|Keyword|0
using|Keyword|0
var|Keyword|0
void|Keyword|0
while|Keyword|0
with|Keyword|0
yield|Keyword|0"#,
        Some(SORT_TEXT_GLOBALS_OR_KEYWORDS),
    )
}

pub fn completion_global_type_decls() -> Vec<CompletionsExpectedItem> {
    items_from_specs(
        r#"Symbol|Interface|0
PropertyKey|Class|0
PropertyDescriptor|Interface|0
PropertyDescriptorMap|Interface|0
Object|Variable|0
ObjectConstructor|Interface|0
Function|Variable|0
FunctionConstructor|Interface|0
ThisParameterType|Class|0
OmitThisParameter|Class|0
CallableFunction|Interface|0
NewableFunction|Interface|0
IArguments|Interface|0
String|Variable|0
StringConstructor|Interface|0
Boolean|Variable|0
BooleanConstructor|Interface|0
Number|Variable|0
NumberConstructor|Interface|0
TemplateStringsArray|Interface|0
ImportMeta|Interface|0
ImportCallOptions|Interface|0
ImportAssertions|Interface|1
ImportAttributes|Interface|0
Math|Variable|0
Date|Variable|0
DateConstructor|Interface|0
RegExpMatchArray|Interface|0
RegExpExecArray|Interface|0
RegExp|Variable|0
RegExpConstructor|Interface|0
Error|Variable|0
ErrorConstructor|Interface|0
EvalError|Variable|0
EvalErrorConstructor|Interface|0
RangeError|Variable|0
RangeErrorConstructor|Interface|0
ReferenceError|Variable|0
ReferenceErrorConstructor|Interface|0
SyntaxError|Variable|0
SyntaxErrorConstructor|Interface|0
TypeError|Variable|0
TypeErrorConstructor|Interface|0
URIError|Variable|0
URIErrorConstructor|Interface|0
JSON|Variable|0
ReadonlyArray|Interface|0
ConcatArray|Interface|0
Array|Variable|0
ArrayConstructor|Interface|0
TypedPropertyDescriptor|Interface|0
ClassDecorator|Class|0
PropertyDecorator|Class|0
MethodDecorator|Class|0
ParameterDecorator|Class|0
ClassMemberDecoratorContext|Class|0
DecoratorContext|Class|0
DecoratorMetadata|Class|0
DecoratorMetadataObject|Class|0
ClassDecoratorContext|Interface|0
ClassMethodDecoratorContext|Interface|0
ClassGetterDecoratorContext|Interface|0
ClassSetterDecoratorContext|Interface|0
ClassAccessorDecoratorContext|Interface|0
ClassAccessorDecoratorTarget|Interface|0
ClassAccessorDecoratorResult|Interface|0
ClassFieldDecoratorContext|Interface|0
PromiseConstructorLike|Class|0
PromiseLike|Interface|0
Promise|Interface|0
Awaited|Class|0
ArrayLike|Interface|0
Partial|Class|0
Required|Class|0
Readonly|Class|0
Pick|Class|0
Record|Class|0
Exclude|Class|0
Extract|Class|0
Omit|Class|0
NonNullable|Class|0
Parameters|Class|0
ConstructorParameters|Class|0
ReturnType|Class|0
InstanceType|Class|0
Uppercase|Class|0
Lowercase|Class|0
Capitalize|Class|0
Uncapitalize|Class|0
NoInfer|Class|0
ThisType|Interface|0
ArrayBuffer|Variable|0
ArrayBufferTypes|Interface|0
ArrayBufferLike|Class|0
ArrayBufferConstructor|Interface|0
ArrayBufferView|Interface|0
DataView|Variable|0
DataViewConstructor|Interface|0
Int8Array|Variable|0
Int8ArrayConstructor|Interface|0
Uint8Array|Variable|0
Uint8ArrayConstructor|Interface|0
Uint8ClampedArray|Variable|0
Uint8ClampedArrayConstructor|Interface|0
Int16Array|Variable|0
Int16ArrayConstructor|Interface|0
Uint16Array|Variable|0
Uint16ArrayConstructor|Interface|0
Int32Array|Variable|0
Int32ArrayConstructor|Interface|0
Uint32Array|Variable|0
Uint32ArrayConstructor|Interface|0
Float32Array|Variable|0
Float32ArrayConstructor|Interface|0
Float64Array|Variable|0
Float64ArrayConstructor|Interface|0
Intl|Module|0
WeakKey|Class|0
WeakKeyTypes|Interface|0"#,
        Some(SORT_TEXT_GLOBALS_OR_KEYWORDS),
    )
}

pub fn completion_type_keywords() -> Vec<CompletionsExpectedItem> {
    items_from_specs(
        r#"any|Keyword|0
asserts|Keyword|0
bigint|Keyword|0
boolean|Keyword|0
false|Keyword|0
infer|Keyword|0
keyof|Keyword|0
never|Keyword|0
null|Keyword|0
number|Keyword|0
object|Keyword|0
readonly|Keyword|0
string|Keyword|0
symbol|Keyword|0
true|Keyword|0
typeof|Keyword|0
undefined|Keyword|0
unique|Keyword|0
unknown|Keyword|0
void|Keyword|0"#,
        Some(SORT_TEXT_GLOBALS_OR_KEYWORDS),
    )
}

pub fn completion_class_element_keywords() -> Vec<CompletionsExpectedItem> {
    items_from_specs(
        r#"abstract|Keyword|0
accessor|Keyword|0
async|Keyword|0
constructor|Keyword|0
declare|Keyword|0
get|Keyword|0
override|Keyword|0
private|Keyword|0
protected|Keyword|0
public|Keyword|0
readonly|Keyword|0
set|Keyword|0
static|Keyword|0"#,
        Some(SORT_TEXT_GLOBALS_OR_KEYWORDS),
    )
}

pub fn completion_class_element_in_js_keywords() -> Vec<CompletionsExpectedItem> {
    get_in_js_keywords(completion_class_element_keywords())
}

pub fn completion_globals() -> Vec<CompletionsExpectedItem> {
    sort_completion_items(
        completion_global_vars()
            .into_iter()
            .chain(completion_global_keywords())
            .chain([
                completion_global_this_item(),
                completion_undefined_var_item(),
            ])
            .collect(),
    )
}

pub fn sort_completion_items(
    mut items: Vec<CompletionsExpectedItem>,
) -> Vec<CompletionsExpectedItem> {
    items.sort_by(|a, b| {
        let a_sort_text = completion_sort_text(a).unwrap_or(SORT_TEXT_LOCATION_PRIORITY);
        let b_sort_text = completion_sort_text(b).unwrap_or(SORT_TEXT_LOCATION_PRIORITY);
        compare_strings_case_insensitive_then_sensitive(a_sort_text, b_sort_text).then_with(|| {
            compare_strings_case_insensitive_then_sensitive(
                &completion_label(a),
                &completion_label(b),
            )
        })
    });
    items
}

pub fn completion_globals_plus(
    items: Vec<CompletionsExpectedItem>,
    no_lib: bool,
) -> Vec<CompletionsExpectedItem> {
    let mut all = items;
    if no_lib {
        all.extend([
            completion_global_this_item(),
            completion_undefined_var_item(),
        ]);
        all.extend(completion_global_keywords());
    } else {
        all.extend(completion_globals());
    }
    sort_completion_items(all)
}

pub fn completion_global_types_plus(
    items: Vec<CompletionsExpectedItem>,
) -> Vec<CompletionsExpectedItem> {
    let mut all = completion_global_type_decls();
    all.push(completion_global_this_item());
    all.extend(completion_type_keywords());
    all.extend(items);
    sort_completion_items(all)
}

pub fn completion_global_types() -> Vec<CompletionsExpectedItem> {
    completion_global_types_plus(Vec::new())
}

pub fn get_in_js_keywords(keywords: Vec<CompletionsExpectedItem>) -> Vec<CompletionsExpectedItem> {
    keywords
        .into_iter()
        .filter(|item| {
            !matches!(
                completion_label(item).as_str(),
                "enum"
                    | "interface"
                    | "implements"
                    | "private"
                    | "protected"
                    | "public"
                    | "abstract"
                    | "any"
                    | "boolean"
                    | "declare"
                    | "infer"
                    | "is"
                    | "keyof"
                    | "module"
                    | "namespace"
                    | "never"
                    | "readonly"
                    | "number"
                    | "object"
                    | "string"
                    | "symbol"
                    | "type"
                    | "unique"
                    | "override"
                    | "unknown"
                    | "global"
                    | "bigint"
            )
        })
        .collect()
}

pub fn completion_global_in_js_keywords() -> Vec<CompletionsExpectedItem> {
    get_in_js_keywords(completion_global_keywords())
}

pub fn completion_globals_in_js_plus(
    items: Vec<CompletionsExpectedItem>,
    no_lib: bool,
) -> Vec<CompletionsExpectedItem> {
    let mut all = items;
    all.extend([
        completion_global_this_item(),
        completion_undefined_var_item(),
    ]);
    all.extend(completion_global_in_js_keywords());
    if !no_lib {
        all.extend(completion_global_vars());
    }
    sort_completion_items(all)
}

pub fn completion_constructor_parameter_keywords() -> Vec<CompletionsExpectedItem> {
    items_from_specs(
        r#"override|Keyword|0
private|Keyword|0
protected|Keyword|0
public|Keyword|0
readonly|Keyword|0"#,
        Some(SORT_TEXT_GLOBALS_OR_KEYWORDS),
    )
}

pub fn completion_function_members() -> Vec<CompletionsExpectedItem> {
    items_from_specs(
        r#"apply|Method|0
arguments|Field|0
bind|Method|0
call|Method|0
caller|Field|0
length|Field|0
toString|Method|0"#,
        None,
    )
}

pub fn completion_function_members_plus(
    items: Vec<CompletionsExpectedItem>,
) -> Vec<CompletionsExpectedItem> {
    let mut all = completion_function_members();
    all.extend(items);
    sort_completion_items(all)
}

pub fn completion_function_members_with_prototype() -> Vec<CompletionsExpectedItem> {
    let mut all = completion_function_members();
    all.push(item(
        "prototype",
        lsproto::CompletionItemKind::FIELD,
        None,
        false,
    ));
    sort_completion_items(all)
}

pub fn completion_function_members_with_prototype_plus(
    items: Vec<CompletionsExpectedItem>,
) -> Vec<CompletionsExpectedItem> {
    let mut all = completion_function_members_with_prototype();
    all.extend(items);
    sort_completion_items(all)
}

pub fn completion_type_keywords_plus(
    items: Vec<CompletionsExpectedItem>,
) -> Vec<CompletionsExpectedItem> {
    let mut all = completion_type_keywords();
    all.extend(items);
    sort_completion_items(all)
}

pub fn completion_type_assertion_keywords() -> Vec<CompletionsExpectedItem> {
    completion_global_types_plus(vec![item(
        "const",
        lsproto::CompletionItemKind::KEYWORD,
        Some(SORT_TEXT_GLOBALS_OR_KEYWORDS.to_string()),
        false,
    )])
}

pub fn to_any<T: 'static>(items: Vec<T>) -> Vec<Box<dyn std::any::Any>> {
    items
        .into_iter()
        .map(|item| Box::new(item) as Box<dyn std::any::Any>)
        .collect()
}

fn completion_label(item: &CompletionsExpectedItem) -> String {
    match item {
        CompletionsExpectedItem::Label(label) => label.clone(),
        CompletionsExpectedItem::Item(item) => item.label.clone(),
    }
}

fn completion_sort_text(item: &CompletionsExpectedItem) -> Option<&str> {
    match item {
        CompletionsExpectedItem::Item(item) => item.sort_text.as_deref(),
        CompletionsExpectedItem::Label(_) => None,
    }
}

fn compare_strings_case_insensitive_then_sensitive(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_ascii_lowercase()
        .cmp(&b.to_ascii_lowercase())
        .then_with(|| a.cmp(b))
}

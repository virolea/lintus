//! Loads the config file's YAML the way Ruby's Psych does, so config files
//! written for the Ruby lintus mean the same thing here.
//!
//! yaml-rust2 does the parsing, but its values follow YAML 1.2. Psych follows
//! YAML 1.1 instead: `yes`, `no`, `on` and `off` are booleans, integers may
//! contain `_` and `,` and have `0x`, `0b` and leading-`0` octal forms, a
//! leading `:` makes a symbol, `<<` merges mappings, and a repeated key keeps
//! the last value instead of being an error. This module builds values from
//! the parser's events with Psych's rules.

use std::collections::HashMap;
use std::fmt::Write;

use yaml_rust2::parser::{Event, MarkedEventReceiver, Parser, Tag};
use yaml_rust2::scanner::{Marker, TScalarStyle};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Seq(Vec<Value>),
    Map(Map),
}

/// A mapping that keeps insertion order. Setting a key that is already
/// present replaces its value in place, as a Ruby Hash does.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Map(Vec<(Value, Value)>);

impl Map {
    pub fn insert(&mut self, key: Value, value: Value) {
        match self.0.iter_mut().find(|(existing, _)| *existing == key) {
            Some(entry) => entry.1 = value,
            None => self.0.push((key, value)),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &(Value, Value)> {
        self.0.iter()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Parses the first document of `source`. An empty document is `Null`.
pub fn load(source: &str) -> Result<Value, String> {
    let mut builder = Builder::default();
    let mut parser = Parser::new_from_str(source);
    parser.load(&mut builder, false).map_err(|e| e.to_string())?;
    match builder.error {
        Some(error) => Err(error),
        None => Ok(builder.root.unwrap_or(Value::Null)),
    }
}

impl Value {
    /// Ruby's `to_s`.
    pub fn to_s(&self) -> String {
        match self {
            Value::Null => String::new(),
            Value::Str(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => crate::format::ruby_float(*f),
            Value::Seq(_) | Value::Map(_) => self.inspect(),
        }
    }

    /// Ruby's `inspect`, as printed in error messages.
    pub fn inspect(&self) -> String {
        match self {
            Value::Null => "nil".into(),
            Value::Str(s) => inspect_str(s),
            Value::Seq(items) => format!("[{}]", items.iter().map(Value::inspect).collect::<Vec<_>>().join(", ")),
            Value::Map(map) => {
                if map.is_empty() {
                    return "{}".into();
                }
                let pairs: Vec<_> = map.iter().map(|(k, v)| format!("{} => {}", k.inspect(), v.inspect())).collect();
                format!("{{{}}}", pairs.join(", "))
            }
            other => other.to_s(),
        }
    }

    /// The name of the Ruby class this value would have.
    pub fn class_name(&self) -> &'static str {
        match self {
            Value::Null => "NilClass",
            Value::Bool(true) => "TrueClass",
            Value::Bool(false) => "FalseClass",
            Value::Int(_) => "Integer",
            Value::Float(_) => "Float",
            Value::Str(_) => "String",
            Value::Seq(_) => "Array",
            Value::Map(_) => "Hash",
        }
    }
}

fn inspect_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '#' => out.push('#'),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[derive(Default)]
struct Builder {
    stack: Vec<Frame>,
    anchors: HashMap<usize, Value>,
    root: Option<Value>,
    error: Option<String>,
}

enum Frame {
    Seq { items: Vec<Value>, anchor: usize },
    Map { pairs: Vec<(Value, Value)>, key: Option<Value>, anchor: usize },
}

impl MarkedEventReceiver for Builder {
    fn on_event(&mut self, event: Event, _mark: Marker) {
        match event {
            Event::Scalar(value, style, anchor, tag) => self.push(scalar(value, style, tag.as_ref()), anchor),
            Event::Alias(id) => match self.anchors.get(&id) {
                Some(value) => self.push(value.clone(), 0),
                None => {
                    self.error.get_or_insert(format!("unknown alias {id}"));
                }
            },
            Event::SequenceStart(anchor, _) => self.stack.push(Frame::Seq { items: Vec::new(), anchor }),
            Event::MappingStart(anchor, _) => self.stack.push(Frame::Map { pairs: Vec::new(), key: None, anchor }),
            Event::SequenceEnd => {
                if let Some(Frame::Seq { items, anchor }) = self.stack.pop() {
                    self.push(Value::Seq(items), anchor);
                }
            }
            Event::MappingEnd => {
                if let Some(Frame::Map { pairs, anchor, .. }) = self.stack.pop() {
                    self.push(Value::Map(revive_map(pairs)), anchor);
                }
            }
            _ => {}
        }
    }
}

impl Builder {
    fn push(&mut self, value: Value, anchor: usize) {
        if anchor > 0 {
            self.anchors.insert(anchor, value.clone());
        }
        match self.stack.last_mut() {
            None => {
                self.root.get_or_insert(value);
            }
            Some(Frame::Seq { items, .. }) => items.push(value),
            Some(Frame::Map { pairs, key, .. }) => match key.take() {
                None => *key = Some(value),
                Some(k) => pairs.push((k, value)),
            },
        }
    }
}

/// Builds a mapping from its pairs in order, applying `<<` merge keys the way
/// Psych does: a merged mapping overrides the keys before it, and the keys
/// after it override it.
fn revive_map(pairs: Vec<(Value, Value)>) -> Map {
    let mut map = Map::default();
    for (key, value) in pairs {
        if key != Value::Str("<<".into()) {
            map.insert(key, value);
            continue;
        }
        match value {
            Value::Map(merged) => {
                for (k, v) in merged.0 {
                    map.insert(k, v);
                }
            }
            Value::Seq(items) if items.iter().all(|item| matches!(item, Value::Map(_))) => {
                let mut merged = Map::default();
                for item in items.into_iter().rev() {
                    if let Value::Map(m) = item {
                        for (k, v) in m.0 {
                            merged.insert(k, v);
                        }
                    }
                }
                for (k, v) in merged.0 {
                    map.insert(k, v);
                }
            }
            other => map.insert(key, other),
        }
    }
    map
}

fn scalar(value: String, style: TScalarStyle, tag: Option<&Tag>) -> Value {
    if let Some(tag) = tag.filter(|tag| tag.handle == "tag:yaml.org,2002:") {
        return match tag.suffix.as_str() {
            "str" => Value::Str(value),
            "null" => Value::Null,
            "bool" => resolve_plain(&value),
            "int" => parse_int(&value).map_or(Value::Str(value), Value::Int),
            "float" => match resolve_plain(&value) {
                Value::Int(i) => Value::Float(i as f64),
                other => other,
            },
            _ => Value::Str(value),
        };
    }
    if style != TScalarStyle::Plain {
        return Value::Str(value);
    }
    resolve_plain(&value)
}

/// Psych's ScalarScanner#tokenize, for the types a config file can hold.
/// Dates and times, which Psych refuses to load safely, stay strings.
fn resolve_plain(s: &str) -> Value {
    if s.is_empty() || s == "~" || s.eq_ignore_ascii_case("null") {
        return Value::Null;
    }
    for word in ["yes", "true", "on"] {
        if s.eq_ignore_ascii_case(word) {
            return Value::Bool(true);
        }
    }
    for word in ["no", "false", "off"] {
        if s.eq_ignore_ascii_case(word) {
            return Value::Bool(false);
        }
    }
    let (sign, unsigned) = match s.strip_prefix('-') {
        Some(rest) => (-1.0, rest),
        None => (1.0, s.strip_prefix('+').unwrap_or(s)),
    };
    if unsigned.eq_ignore_ascii_case(".inf") {
        return Value::Float(sign * f64::INFINITY);
    }
    if s.eq_ignore_ascii_case(".nan") {
        return Value::Float(f64::NAN);
    }
    if let Some(symbol) = s.strip_prefix(':').filter(|rest| !rest.is_empty()) {
        let unquoted = [('"', '"'), ('\'', '\'')]
            .iter()
            .find_map(|(open, close)| symbol.strip_prefix(*open)?.strip_suffix(*close));
        return Value::Str(unquoted.unwrap_or(symbol).to_string());
    }
    if let Some(float) = parse_float(s) {
        return float;
    }
    if let Some(int) = parse_int(s) {
        return Value::Int(int);
    }
    Value::Str(s.to_string())
}

/// Psych's FLOAT: `[-+]?([0-9][0-9_,]*)?\.[0-9]*([eE][-+][0-9]+)?`
fn parse_float(s: &str) -> Option<Value> {
    let body = s.strip_prefix(['+', '-']).unwrap_or(s);
    let (int_part, rest) = body.split_once('.')?;
    if !int_part.is_empty()
        && !(int_part.starts_with(|c: char| c.is_ascii_digit())
            && int_part.chars().all(|c| c.is_ascii_digit() || c == '_' || c == ','))
    {
        return None;
    }
    let (fraction, exponent) = match rest.find(['e', 'E']) {
        Some(i) => (&rest[..i], Some(&rest[i + 1..])),
        None => (rest, None),
    };
    if !fraction.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if let Some(exponent) = exponent {
        let digits = exponent.strip_prefix(['+', '-'])?;
        if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
    }
    if int_part.is_empty() && fraction.is_empty() && exponent.is_none() {
        return Some(Value::Str(s.to_string())); // a lone "." or "-."
    }
    let cleaned: String = s.chars().filter(|&c| c != '_' && c != ',').collect();
    cleaned.parse::<f64>().ok().map(Value::Float)
}

/// Psych's INTEGER_LEGACY: binary, octal (leading 0), decimal and hex, with `_` and `,` allowed.
fn parse_int(s: &str) -> Option<i64> {
    let (negative, body) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let separator = |c: char| c == '_' || c == ',';
    let digits_in = |text: &str, radix: u32| -> Option<String> {
        let first = text.trim_start_matches(separator);
        if !first.starts_with(|c: char| c.is_digit(radix)) || !text.chars().all(|c| c.is_digit(radix) || separator(c)) {
            return None;
        }
        Some(text.chars().filter(|&c| !separator(c)).collect())
    };

    let (digits, radix) = if let Some(bin) = body.strip_prefix("0b") {
        (digits_in(bin, 2)?, 2)
    } else if let Some(hex) = body.strip_prefix("0x") {
        (digits_in(hex, 16)?, 16)
    } else if body == "0" {
        ("0".to_string(), 10)
    } else if let Some(octal) = body.strip_prefix('0') {
        (digits_in(octal, 8)?, 8)
    } else {
        // Decimal: a separator must sit between two digits.
        let chars: Vec<char> = body.chars().collect();
        let valid = chars.first().is_some_and(|c| c.is_ascii_digit())
            && chars.iter().enumerate().all(|(i, &c)| {
                c.is_ascii_digit() || (separator(c) && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit()))
            });
        if !valid {
            return None;
        }
        (chars.into_iter().filter(|&c| !separator(c)).collect(), 10)
    };
    let value = i64::from_str_radix(&digits, radix).ok()?;
    Some(if negative { -value } else { value })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(yaml: &str) -> Value {
        match load(&format!("v: {yaml}\n")).unwrap() {
            Value::Map(map) => map.iter().next().unwrap().1.clone(),
            other => panic!("{other:?}"),
        }
    }

    fn s(text: &str) -> Value {
        Value::Str(text.into())
    }

    #[test]
    fn plain_scalars_are_typed_like_psych() {
        assert_eq!(one(""), Value::Null);
        assert_eq!(one("~"), Value::Null);
        assert_eq!(one("NULL"), Value::Null);
        for truthy in ["true", "True", "yes", "YES", "on", "On"] {
            assert_eq!(one(truthy), Value::Bool(true), "{truthy}");
        }
        for falsy in ["false", "no", "No", "off", "OFF"] {
            assert_eq!(one(falsy), Value::Bool(false), "{falsy}");
        }
        assert_eq!(one("y"), s("y"));
        assert_eq!(one("42"), Value::Int(42));
        assert_eq!(one("-42"), Value::Int(-42));
        assert_eq!(one("100_000"), Value::Int(100_000));
        assert_eq!(one("1,000"), Value::Int(1000));
        assert_eq!(one("0x1f"), Value::Int(31));
        assert_eq!(one("0b101"), Value::Int(5));
        assert_eq!(one("017"), Value::Int(15));
        assert_eq!(one("09"), s("09"));
        assert_eq!(one("1_"), s("1_"));
        assert_eq!(one("0.7"), Value::Float(0.7));
        assert_eq!(one(".5"), Value::Float(0.5));
        assert_eq!(one("1."), Value::Float(1.0));
        assert_eq!(one("1.5e+3"), Value::Float(1500.0));
        assert_eq!(one("1e3"), s("1e3"));
        assert_eq!(one("-.inf"), Value::Float(f64::NEG_INFINITY));
        assert!(matches!(one(".NaN"), Value::Float(f) if f.is_nan()));
        assert_eq!(one(":sym"), s("sym"));
        assert_eq!(one("2024-01-01"), s("2024-01-01"));
        assert_eq!(one("app/**/*.rb"), s("app/**/*.rb"));
    }

    #[test]
    fn quoted_and_tagged_scalars() {
        assert_eq!(one("\"yes\""), s("yes"));
        assert_eq!(one("'42'"), s("42"));
        assert_eq!(one("!!str 42"), s("42"));
        assert_eq!(one("!!float 1"), Value::Float(1.0));
    }

    #[test]
    fn repeated_keys_keep_the_first_position_and_the_last_value() {
        let Value::Map(map) = load("a: 1\nb: 2\na: 3\n").unwrap() else { panic!() };
        assert_eq!(map.0, vec![(s("a"), Value::Int(3)), (s("b"), Value::Int(2))]);
    }

    #[test]
    fn anchors_aliases_and_merge_keys() {
        let yaml = "base: &base\n  a: 1\n  b: 2\nother: &other\n  c: 3\none:\n  b: 0\n  <<: *base\n  a: 9\nmany:\n  <<: [*other, *base]\nlist: &l [x]\nsame: *l\n";
        let Value::Map(map) = load(yaml).unwrap() else { panic!() };
        let get = |key: &str| map.iter().find(|(k, _)| *k == s(key)).unwrap().1.clone();

        let Value::Map(one) = get("one") else { panic!() };
        assert_eq!(one.0, vec![(s("b"), Value::Int(2)), (s("a"), Value::Int(9))]);
        let Value::Map(many) = get("many") else { panic!() };
        assert_eq!(many.len(), 3);
        assert_eq!(get("same"), Value::Seq(vec![s("x")]));
    }

    #[test]
    fn only_the_first_document_is_read() {
        assert_eq!(load("a: 1\n---\nb: 2\n").unwrap(), Value::Map(Map(vec![(s("a"), Value::Int(1))])));
        assert_eq!(load("").unwrap(), Value::Null);
        assert_eq!(load("# just a comment\n").unwrap(), Value::Null);
    }

    #[test]
    fn syntax_errors_are_reported() {
        assert!(load("rules: [\n").is_err());
    }

    #[test]
    fn to_s_and_inspect_follow_ruby() {
        assert_eq!(Value::Null.to_s(), "");
        assert_eq!(Value::Float(1.0).to_s(), "1.0");
        assert_eq!(Value::Str("just a string".into()).inspect(), "\"just a string\"");
        assert_eq!(Value::Seq(vec![Value::Int(1), Value::Null]).inspect(), "[1, nil]");
        assert_eq!(Value::Map(Map(vec![(s("a"), Value::Bool(true))])).inspect(), "{\"a\" => true}");
    }
}

//! Gestalts: versioned, data-defined shapes a note may have (`gestalts/<name>/v<N>.yml`).
//!
//! A note is a *candidate* for a gestalt when it carries all `requires_tags` and, if
//! `trigger_fields` is set, at least one of them. A candidate *has* the gestalt when every rule
//! holds; otherwise the broken rules are reported as lints.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

use crate::note::{Note, Ref};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GestaltDef {
    pub name: String,
    pub version: u32,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub requires_tags: Vec<String>,
    #[serde(default)]
    pub trigger_fields: Vec<String>,
    #[serde(default)]
    pub fields: BTreeMap<String, FieldRule>,
    /// by label; `_` counts unlabelled ones
    #[serde(default)]
    pub blobs: BTreeMap<String, Count>,
    #[serde(default)]
    pub relationships: BTreeMap<String, Count>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldRule {
    #[serde(rename = "type", default)]
    pub ty: FieldType,
    #[serde(default)]
    pub required: bool,
    /// numeric bounds (inclusive), or item count bounds for lists
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub one_of: Option<Vec<Value>>,
    /// for `type: map`
    #[serde(default)]
    pub fields: BTreeMap<String, FieldRule>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    #[default]
    Any,
    /// single-line string
    String,
    /// any string, usually multi-line
    Text,
    Number,
    Integer,
    Bool,
    /// `YYYY-MM-DD`
    Date,
    /// ISO 8601 `YYYY-MM-DDTHH:MM...`
    Datetime,
    Url,
    StringList,
    Map,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Count {
    #[serde(default)]
    pub min: Option<usize>,
    #[serde(default)]
    pub max: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Lint {
    /// `name@version`
    pub gestalt: String,
    /// e.g. `fsrs.due`, `blobs.audio`
    pub path: String,
    pub message: String,
}

impl GestaltDef {
    pub fn id(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }

    pub fn is_candidate(&self, note: &Note) -> bool {
        self.requires_tags.iter().all(|t| note.has_tag(t))
            && (self.trigger_fields.is_empty() || self.trigger_fields.iter().any(|f| note.get(f).is_some()))
    }

    /// `None`: not a candidate. `Some([])`: the note has this gestalt.
    pub fn check(&self, note: &Note) -> Option<Vec<Lint>> {
        if !self.is_candidate(note) {
            return None;
        }
        let mut lints = Vec::new();
        let mut lint = |path: String, message: String| lints.push(Lint { gestalt: self.id(), path, message });
        if note.value.as_ref().is_none_or(|v| !v.is_mapping()) {
            lint(String::new(), "note is not a YAML map".into());
            return Some(lints);
        }
        for (field, rule) in &self.fields {
            check_field(field, note.get(field), rule, &mut lint);
        }
        check_counts("blobs", &note.blobs(), &self.blobs, &mut lint);
        check_counts("relationships", &note.relationships(), &self.relationships, &mut lint);
        Some(lints)
    }
}

fn check_field(path: &str, v: Option<&Value>, rule: &FieldRule, lint: &mut impl FnMut(String, String)) {
    let Some(v) = v.filter(|v| !v.is_null()) else {
        if rule.required {
            lint(path.into(), "is required".into());
        }
        return;
    };
    let fail = |lint: &mut dyn FnMut(String, String), what: &str| lint(path.into(), format!("must be {what}"));
    let ok = match rule.ty {
        FieldType::Any => true,
        FieldType::String => v.as_str().is_some_and(|s| !s.contains('\n')),
        FieldType::Text => v.is_string(),
        FieldType::Number => v.is_number(),
        FieldType::Integer => v.is_i64() || v.is_u64(),
        FieldType::Bool => v.is_bool(),
        FieldType::Date => v.as_str().is_some_and(is_date),
        FieldType::Datetime => v.as_str().is_some_and(is_datetime),
        FieldType::Url => v.as_str().is_some_and(is_url),
        FieldType::StringList => v.as_sequence().is_some_and(|s| s.iter().all(Value::is_string)),
        FieldType::Map => v.is_mapping(),
    };
    if !ok {
        fail(lint, type_name(rule.ty));
        return;
    }
    let measure = match v {
        Value::Number(n) => n.as_f64(),
        Value::Sequence(s) => Some(s.len() as f64),
        _ => None,
    };
    if let Some(m) = measure {
        if rule.min.is_some_and(|min| m < min) {
            lint(path.into(), format!("must be at least {}", rule.min.unwrap()));
        }
        if rule.max.is_some_and(|max| m > max) {
            lint(path.into(), format!("must be at most {}", rule.max.unwrap()));
        }
    }
    if let Some(options) = &rule.one_of
        && !options.contains(v)
    {
        let shown: Vec<String> = options.iter().map(|o| crate::note::to_yaml(o).trim().to_owned()).collect();
        lint(path.into(), format!("must be one of {}", shown.join(", ")));
    }
    if let Value::Mapping(m) = v {
        for (sub, sub_rule) in &rule.fields {
            check_field(&format!("{path}.{sub}"), m.get(sub.as_str()), sub_rule, lint);
        }
    }
}

fn check_counts(kind: &str, refs: &[Ref], rules: &BTreeMap<String, Count>, lint: &mut impl FnMut(String, String)) {
    for (label, count) in rules {
        let n = refs.iter().filter(|r| r.label.as_deref().unwrap_or("_") == label).count();
        if count.min.is_some_and(|min| n < min) {
            lint(format!("{kind}.{label}"), format!("needs at least {} `{label}`", count.min.unwrap()));
        }
        if count.max.is_some_and(|max| n > max) {
            lint(format!("{kind}.{label}"), format!("allows at most {} `{label}`", count.max.unwrap()));
        }
    }
}

fn type_name(t: FieldType) -> &'static str {
    match t {
        FieldType::Any => "anything",
        FieldType::String => "a single-line string",
        FieldType::Text => "a string",
        FieldType::Number => "a number",
        FieldType::Integer => "an integer",
        FieldType::Bool => "true or false",
        FieldType::Date => "a date (YYYY-MM-DD)",
        FieldType::Datetime => "a datetime (YYYY-MM-DDTHH:MM…)",
        FieldType::Url => "a URL",
        FieldType::StringList => "a list of strings",
        FieldType::Map => "a map",
    }
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let num = |r: std::ops::Range<usize>| s[r].parse::<u32>().ok();
    matches!((num(0..4), num(5..7), num(8..10)), (Some(_), Some(m @ 1..=12), Some(d @ 1..=31)) if m > 0 && d > 0)
}

fn is_datetime(s: &str) -> bool {
    s.len() >= 16 && is_date(&s[..10]) && s.as_bytes()[10] == b'T' && s[11..13].parse::<u32>().is_ok_and(|h| h < 24)
}

fn is_url(s: &str) -> bool {
    let rest = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://"));
    rest.is_some_and(|r| {
        let host = r.split(['/', '?', '#']).next().unwrap_or("");
        !host.is_empty() && !r.contains(char::is_whitespace)
    })
}

/// All known gestalt definitions.
#[derive(Debug, Clone)]
pub struct Gestalts {
    defs: Vec<GestaltDef>,
}

/// The definitions shipped in `gestalts/`, embedded at build time.
const BUILTIN: &[&str] = &[
    include_str!("../../../gestalts/flashcard/v1.yml"),
    include_str!("../../../gestalts/appointment/v1.yml"),
    include_str!("../../../gestalts/link/v1.yml"),
    include_str!("../../../gestalts/pronunciation-card/v1.yml"),
    include_str!("../../../gestalts/fsrs-state/v1.yml"),
];

impl Gestalts {
    pub fn builtin() -> Self {
        Self::from_yaml(BUILTIN).expect("builtin gestalts are valid")
    }

    pub fn from_yaml(sources: &[&str]) -> Result<Self, String> {
        let defs = sources
            .iter()
            .map(|s| serde_yaml_ng::from_str::<GestaltDef>(s).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Gestalts { defs })
    }

    pub fn defs(&self) -> &[GestaltDef] {
        &self.defs
    }

    /// by `name@version`
    pub fn get(&self, id: &str) -> Option<&GestaltDef> {
        self.defs.iter().find(|d| d.id() == id)
    }

    /// The gestalts the note has.
    pub fn of(&self, note: &Note) -> Vec<String> {
        self.defs.iter().filter(|d| d.check(note).is_some_and(|l| l.is_empty())).map(GestaltDef::id).collect()
    }

    /// Broken rules of every gestalt the note is a candidate for.
    pub fn lint(&self, note: &Note) -> Vec<Lint> {
        self.defs.iter().filter_map(|d| d.check(note)).flatten().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(raw: &str) -> Note {
        Note::parse("k", raw)
    }

    #[test]
    fn builtins_load() {
        let g = Gestalts::builtin();
        assert!(g.get("flashcard@1").is_some());
        assert!(g.get("fsrs-state@1").is_some());
        assert!(g.get("flashcard@2").is_none());
    }

    #[test]
    fn a_note_can_have_several_gestalts() {
        let g = Gestalts::builtin();
        let n = note("tags: [appointment]\ndate: 2026-11-03\nurl: https://example.com/meet\n");
        assert_eq!(g.of(&n), vec!["appointment@1", "link@1"]);
        assert!(g.lint(&n).is_empty());
    }

    #[test]
    fn candidates_get_lints() {
        let g = Gestalts::builtin();
        let n = note("tags: [flashcard]\nfront: hola\n");
        assert!(g.of(&n).is_empty());
        let lints = g.lint(&n);
        assert_eq!(lints.len(), 1);
        assert_eq!(lints[0].path, "back");
        // not tagged: no lints, no gestalt
        assert!(g.lint(&note("front: hola")).is_empty());
        assert!(g.lint(&note("url: not a url")).iter().any(|l| l.gestalt == "link@1"));
        assert!(g.lint(&note("tags: [appointment]\ndate: 2026-13-01")).iter().any(|l| l.path == "date"));
    }

    #[test]
    fn pronunciation_card_and_fsrs_state() {
        let g = Gestalts::builtin();
        let card = note("title: δελφίς\nlang: grc\ntags: [pronunciation-card]\nblobs:\n  - {label: image, file: a.webp}\n  - {label: audio, file: a.mp3}\n");
        assert_eq!(g.of(&card), vec!["pronunciation-card@1"]);
        let no_audio = note("title: x\ntags: [pronunciation-card]\n");
        assert_eq!(g.lint(&no_audio)[0].path, "blobs.audio");

        let state = note(
            "tags: [fsrs-state]\nrelationships: [{to: abc, label: reviews}]\nmodel: FSRS-7\nlast_rating: 3\ncard:\n  dueAt: 2026-01-05T18:39:55.164Z\n  lastReviewAt: null\n  stability: 3.92\n  stabilityFast: 3.13\n  difficulty: 3.53\n  state: 2\n  scheduleStatus: review\n  reps: 1\n  lapses: 0\n  scheduledDays: 4.77\n  learningStep: 0\n",
        );
        assert_eq!(g.of(&state), vec!["fsrs-state@1"], "{:?}", g.lint(&state));
        let bad = note("tags: [fsrs-state]\nmodel: FSRS-7\nlast_rating: 7\ncard: {dueAt: tomorrow}\n");
        let paths: Vec<_> = g.lint(&bad).into_iter().map(|l| l.path).collect();
        assert!(paths.contains(&"last_rating".to_string()));
        assert!(paths.contains(&"card.dueAt".to_string()));
        assert!(paths.contains(&"relationships.reviews".to_string()));
    }
}

//! Three-way merge of one note, and syncing two storages against a base snapshot.
//!
//! Rules, kept simple on purpose:
//! - one side changed (vs. the base): take it
//! - both changed, both YAML maps: merge per top-level key; a key both sides changed differently
//!   is a conflict, resolved local-first
//! - except `relationships`, `blobs` and `tags`, merged as sets: an item either side added is kept, an item either side
//!   removed is dropped, local order first, then remote additions. Items are compared whole, so a label changed
//!   differently on both sides leaves both items.
//! - both changed otherwise (unparseable, not maps, created on both sides without a base): local wins
//! - a delete on one side and an edit on the other: the edit wins
//!
//! On a conflict the remote text is kept as a separate note `<key>.conflict-<tag>`.

use std::collections::{BTreeSet, HashMap};

use serde::Serialize;
use serde_yaml_ng::{Mapping, Value};

use crate::note::{parse_yaml, to_yaml};
use crate::storage::{Result, Storage};

#[derive(Debug, PartialEq)]
pub struct Merged {
    /// `None`: the note is deleted
    pub text: Option<String>,
    /// the remote text, when part of it lost a conflict
    pub conflict: Option<String>,
}

pub fn merge3(base: Option<&str>, local: Option<&str>, remote: Option<&str>) -> Merged {
    let take = |t: Option<&str>| Merged { text: t.map(str::to_owned), conflict: None };
    if local == remote || remote == base {
        return take(local);
    }
    if local == base {
        return take(remote);
    }
    // both changed
    let (Some(l), Some(r)) = (local, remote) else {
        return take(local.or(remote)); // one deleted, the other edited
    };
    let maps = (base.map(parse_yaml), parse_yaml(l), parse_yaml(r));
    let (Some(Ok(Value::Mapping(b))), Ok(Value::Mapping(lm)), Ok(Value::Mapping(rm))) = maps else {
        return Merged { text: Some(l.to_owned()), conflict: Some(r.to_owned()) };
    };
    let (merged, conflicted) = merge_maps(&b, &lm, &rm);
    if merged == lm {
        // nothing came over from remote
        return Merged { text: Some(l.to_owned()), conflict: conflicted.then(|| r.to_owned()) };
    }
    Merged { text: Some(to_yaml(&Value::Mapping(merged))), conflict: conflicted.then(|| r.to_owned()) }
}

fn merge_maps(base: &Mapping, local: &Mapping, remote: &Mapping) -> (Mapping, bool) {
    let mut out = Mapping::new();
    let mut conflicted = false;
    let keys = local.keys().chain(remote.keys().filter(|k| !local.contains_key(*k)));
    for k in keys.cloned().collect::<Vec<_>>() {
        let (b, l, r) = (base.get(&k), local.get(&k), remote.get(&k));
        let pick = if l == r || r == b {
            l.cloned()
        } else if l == b {
            r.cloned()
        } else if let Some(v) = k.as_str().filter(|k| SET_KEYS.contains(k)).and_then(|_| set_merge(b, l, r)) {
            v
        } else {
            conflicted = true;
            l.or(r).cloned()
        };
        if let Some(v) = pick {
            out.insert(k, v);
        }
    }
    (out, conflicted)
}

const SET_KEYS: [&str; 3] = ["relationships", "blobs", "tags"];

/// Merges three lists as sets. `None` when a side isn't a list; `Some(None)` drops the key.
fn set_merge(base: Option<&Value>, local: Option<&Value>, remote: Option<&Value>) -> Option<Option<Value>> {
    fn items(v: Option<&Value>) -> Option<&[Value]> {
        match v {
            None | Some(Value::Null) => Some(&[]),
            Some(Value::Sequence(s)) => Some(s),
            Some(_) => None,
        }
    }
    let (b, l, r) = (items(base)?, items(local)?, items(remote)?);
    let mut out: Vec<Value> = l.iter().filter(|i| !b.contains(i) || r.contains(i)).cloned().collect();
    for i in r {
        if !b.contains(i) && !out.contains(i) {
            out.push(i.clone());
        }
    }
    Some((local.is_some() || !out.is_empty()).then_some(Value::Sequence(out)))
}

#[derive(Debug, Default, Serialize)]
pub struct SyncReport {
    /// notes changed locally because of remote
    pub pulled: Vec<String>,
    /// notes changed remotely because of local
    pub pushed: Vec<String>,
    /// conflict notes created
    pub conflicts: Vec<String>,
    pub blobs_pulled: Vec<String>,
    pub blobs_pushed: Vec<String>,
    /// how many notes each side had before the sync
    pub notes: NoteCounts,
}

#[derive(Debug, Default, Serialize)]
pub struct NoteCounts {
    pub local: usize,
    pub remote: usize,
    pub base: usize,
}

/// The steps `sync_with` reports as it finishes them, in order.
pub const SYNC_STEPS: [&str; 5] = ["read_local", "read_remote", "read_base", "merge_notes", "blobs"];

/// Syncs `local` and `remote` against `base`, the state both had after the last sync.
/// Afterwards all three hold the same notes. Blobs are only ever added, never merged or deleted.
/// `tag` names conflict notes and should be unique per sync, e.g. a timestamp.
pub async fn sync(local: &dyn Storage, remote: &dyn Storage, base: &dyn Storage, tag: &str) -> Result<SyncReport> {
    sync_with(local, remote, base, tag, &mut |_| {}).await
}

/// `sync`, calling `step` with each of `SYNC_STEPS` as it's done (to time them; the core has no clock).
pub async fn sync_with(local: &dyn Storage, remote: &dyn Storage, base: &dyn Storage, tag: &str, step: &mut dyn FnMut(&str)) -> Result<SyncReport> {
    let mut report = SyncReport::default();
    let all = |v: Vec<(String, String)>| v.into_iter().collect::<HashMap<_, _>>();
    let l = all(local.read_all_notes().await?);
    step("read_local");
    let r = all(remote.read_all_notes().await?);
    step("read_remote");
    let b = all(base.read_all_notes().await?);
    step("read_base");
    report.notes = NoteCounts { local: l.len(), remote: r.len(), base: b.len() };
    // sorted, so the report lists keys in order
    let keys: BTreeSet<&str> = l.keys().chain(r.keys()).chain(b.keys()).map(String::as_str).collect();

    for key in keys {
        let (lt, rt, bt) = (l.get(key).map(String::as_str), r.get(key).map(String::as_str), b.get(key).map(String::as_str));
        let m = merge3(bt, lt, rt);
        if m.text.as_deref() != lt {
            put(local, key, m.text.as_deref()).await?;
            report.pulled.push(key.to_owned());
        }
        if m.text.as_deref() != rt {
            put(remote, key, m.text.as_deref()).await?;
            report.pushed.push(key.to_owned());
        }
        if m.text.as_deref() != bt {
            put(base, key, m.text.as_deref()).await?;
        }
        if let Some(c) = m.conflict {
            let ck = format!("{key}.conflict-{tag}");
            for s in [local, remote, base] {
                s.write_note(&ck, &c).await?;
            }
            report.conflicts.push(ck);
        }
    }

    step("merge_notes");

    let (lb, rb) = (local.list_blobs().await?, remote.list_blobs().await?);
    for name in rb.iter().filter(|n| !lb.contains(n)) {
        if let Some(bytes) = remote.read_blob(name).await? {
            local.write_blob(name, &bytes).await?;
            report.blobs_pulled.push(name.clone());
        }
    }
    for name in lb.iter().filter(|n| !rb.contains(n)) {
        if let Some(bytes) = local.read_blob(name).await? {
            remote.write_blob(name, &bytes).await?;
            report.blobs_pushed.push(name.clone());
        }
    }
    step("blobs");
    Ok(report)
}

async fn put(s: &dyn Storage, key: &str, text: Option<&str>) -> Result<()> {
    match text {
        Some(t) => s.write_note(key, t).await,
        None => s.delete_note(key).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Memory;

    fn m(b: Option<&str>, l: Option<&str>, r: Option<&str>) -> Merged {
        merge3(b, l, r)
    }

    #[test]
    fn one_side_changed() {
        assert_eq!(m(Some("a: 1"), Some("a: 2"), Some("a: 1")).text.as_deref(), Some("a: 2"));
        assert_eq!(m(Some("a: 1"), Some("a: 1"), Some("a: 3")).text.as_deref(), Some("a: 3"));
        assert_eq!(m(None, None, Some("new: 1")).text.as_deref(), Some("new: 1"));
        assert_eq!(m(Some("a: 1"), None, Some("a: 1")).text, None);
        assert_eq!(m(Some("a: 1"), Some("a: 1"), None).text, None);
    }

    #[test]
    fn both_changed_different_keys_merge() {
        let r = m(Some("a: 1\nb: 1\n"), Some("a: 2\nb: 1\n"), Some("a: 1\nb: 3\nc: 4\n"));
        assert_eq!(r.text.as_deref(), Some("a: 2\nb: 3\nc: 4\n"));
        assert_eq!(r.conflict, None);
        // a key removed on one side
        let r = m(Some("a: 1\nb: 1\n"), Some("a: 1\n"), Some("a: 1\nb: 1\nc: 1\n"));
        assert_eq!(r.text.as_deref(), Some("a: 1\nc: 1\n"));
    }

    #[test]
    fn same_key_conflicts_local_wins() {
        let r = m(Some("a: 1\nb: 1\n"), Some("a: 2\nb: 1\n"), Some("a: 3\nb: 5\n"));
        assert_eq!(r.text.as_deref(), Some("a: 2\nb: 5\n"));
        assert_eq!(r.conflict.as_deref(), Some("a: 3\nb: 5\n"));
    }

    #[test]
    fn edit_beats_delete() {
        assert_eq!(m(Some("a: 1"), None, Some("a: 2")).text.as_deref(), Some("a: 2"));
        assert_eq!(m(Some("a: 1"), Some("a: 2"), None).text.as_deref(), Some("a: 2"));
    }

    #[test]
    fn unparseable_or_no_base_local_wins() {
        let r = m(Some("a: 1"), Some("a: [x"), Some("a: 2"));
        assert_eq!(r.text.as_deref(), Some("a: [x"));
        assert_eq!(r.conflict.as_deref(), Some("a: 2"));
        let r = m(None, Some("a: 1"), Some("a: 2"));
        assert_eq!(r.text.as_deref(), Some("a: 1"));
        assert!(r.conflict.is_some());
    }

    #[test]
    fn identical_edits_are_no_conflict() {
        assert_eq!(m(Some("a: 1"), Some("a: 2"), Some("a: 2")), Merged { text: Some("a: 2".into()), conflict: None });
    }

    #[test]
    fn set_keys_merge_as_sets() {
        // both add: local order, then remote additions
        let r = m(Some("relationships: [a]\n"), Some("relationships: [a, b]\n"), Some("relationships: [a, c]\n"));
        assert_eq!(r.text.as_deref(), Some("relationships:\n- a\n- b\n- c\n"));
        assert_eq!(r.conflict, None);
        // one removes, the other adds
        let r = m(Some("tags: [x, y]\n"), Some("tags: [y]\n"), Some("tags: [x, y, z]\n"));
        assert_eq!(r.text.as_deref(), Some("tags:\n- y\n- z\n"));
        // both add the same item, and remote removes what local kept
        let r = m(Some("tags: [x]\n"), Some("tags: [x, n]\n"), Some("tags: [n]\n"));
        assert_eq!(r.text.as_deref(), Some("tags:\n- n\n"));
        // labelled items compare whole
        let r = m(
            Some("blobs: [{file: a.webp, label: x}]\n"),
            Some("blobs: [{file: a.webp, label: y}]\n"),
            Some("blobs: [{file: a.webp, label: x}, b.webp]\n"),
        );
        assert_eq!(r.text.as_deref(), Some("blobs:\n- file: a.webp\n  label: y\n- b.webp\n"));
        assert_eq!(r.conflict, None);
    }

    #[test]
    fn set_keys_missing_or_not_lists() {
        // no base key, both add
        let r = m(Some("a: 1\n"), Some("a: 1\ntags: [x]\n"), Some("a: 1\ntags: [y]\n"));
        assert_eq!(r.text.as_deref(), Some("a: 1\ntags:\n- x\n- y\n"));
        // local dropped the key, remote added to it
        let r = m(Some("tags: [x]\n"), Some("{}\n"), Some("tags: [x, y]\n"));
        assert_eq!(r.text.as_deref(), Some("tags:\n- y\n"));
        // local dropped the key, remote removed the rest: stays dropped
        let r = m(Some("a: 1\ntags: [x, y]\n"), Some("a: 1\n"), Some("a: 1\ntags: [x]\n"));
        assert_eq!(r.text.as_deref(), Some("a: 1\n"));
        assert_eq!(r.conflict, None);
        // not a list: a conflict as before
        let r = m(Some("tags: [x]\n"), Some("tags: foo\n"), Some("tags: [x, y]\n"));
        assert_eq!(r.text.as_deref(), Some("tags: foo\n"));
        assert!(r.conflict.is_some());
    }

    #[test]
    fn set_key_merges_beside_a_conflict() {
        let r = m(Some("a: 1\ntags: [x]\n"), Some("a: 2\ntags: [x, l]\n"), Some("a: 3\ntags: [x, r]\n"));
        assert_eq!(r.text.as_deref(), Some("a: 2\ntags:\n- x\n- l\n- r\n"));
        assert_eq!(r.conflict.as_deref(), Some("a: 3\ntags: [x, r]\n"));
    }

    #[test]
    fn sync_converges() {
        pollster::block_on(async {
            let (l, r, b) = (Memory::default(), Memory::default(), Memory::default());
            l.write_note("only-local", "x: 1").await.unwrap();
            r.write_note("only-remote", "y: 1").await.unwrap();
            r.write_blob("img.webp", b"123").await.unwrap();
            let rep = sync(&l, &r, &b, "t1").await.unwrap();
            assert_eq!(rep.pulled, vec!["only-remote"]);
            assert_eq!(rep.pushed, vec!["only-local"]);
            assert_eq!(rep.blobs_pulled, vec!["img.webp"]);

            // now both edit the same note, different keys; one deletes another
            l.write_note("only-local", "x: 2\nz: 1").await.unwrap();
            r.write_note("only-local", "x: 1\nw: 1").await.unwrap();
            r.delete_note("only-remote").await.unwrap();
            let rep = sync(&l, &r, &b, "t2").await.unwrap();
            assert!(rep.conflicts.is_empty());
            let merged = l.read_note("only-local").await.unwrap().unwrap();
            assert_eq!(merged, "x: 2\nz: 1\nw: 1\n");
            assert_eq!(r.read_note("only-local").await.unwrap().unwrap(), merged);
            assert_eq!(l.read_note("only-remote").await.unwrap(), None);

            // a real conflict
            l.write_note("only-local", "x: 3").await.unwrap();
            r.write_note("only-local", "x: 4").await.unwrap();
            let rep = sync(&l, &r, &b, "t3").await.unwrap();
            assert_eq!(rep.conflicts, vec!["only-local.conflict-t3"]);
            assert_eq!(r.read_note("only-local").await.unwrap().as_deref(), Some("x: 3"));
            assert_eq!(l.read_note("only-local.conflict-t3").await.unwrap().as_deref(), Some("x: 4"));

            // both add a relationship: merged, no conflict note
            l.write_note("rel", "relationships: [a]").await.unwrap();
            sync(&l, &r, &b, "t4").await.unwrap();
            l.write_note("rel", "relationships: [a, b]").await.unwrap();
            r.write_note("rel", "relationships: [a, {to: c, label: on-canvas}]").await.unwrap();
            let rep = sync(&l, &r, &b, "t5").await.unwrap();
            assert!(rep.conflicts.is_empty());
            let merged = "relationships:\n- a\n- b\n- to: c\n  label: on-canvas\n";
            assert_eq!(l.read_note("rel").await.unwrap().as_deref(), Some(merged));
            assert_eq!(r.read_note("rel").await.unwrap().as_deref(), Some(merged));

            // nothing changed: a no-op
            let rep = sync(&l, &r, &b, "t6").await.unwrap();
            assert!(rep.pulled.is_empty() && rep.pushed.is_empty() && rep.conflicts.is_empty());
        });
    }
}

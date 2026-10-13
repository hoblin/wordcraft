//! What a command changed, in words, from the document and selection before it ran.

use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{Block, Blocks, Document, InlineObject, Paragraph, Path, StoryRef};

use crate::{Selection, Session};

pub(crate) struct Before {
    doc: Document,
    sel: Selection,
    typing: CharProps,
}

impl Before {
    pub(crate) fn of(s: &Session) -> Self {
        Before { doc: s.doc.clone(), sel: s.sel.clone(), typing: s.typing_props() }
    }
}

pub(crate) fn changes(before: &Before, s: &Session) -> String {
    let (a, b) = (&before.doc, &s.doc);
    let mut said = vec![];
    said.extend(blocks(&a.body, &b.body, ""));
    let parts_changed = a.parts != b.parts;
    said.extend(parts(a, b));
    said.extend(
        field_diff("page setup", &a.last_section, &b.last_section)
            .into_iter()
            .filter(|d| !parts_changed || !d.contains(": headers {") && !d.contains(": footers {")),
    );
    said.extend(styles(a, b));
    said.extend(field_diff("settings", &a.settings, &b.settings));
    said.extend(field_diff("properties", &a.core, &b.core));
    if a.numbering != b.numbering {
        said.push("changed the list definitions".into());
    }
    if a.comments.len() != b.comments.len() {
        said.push(format!("{} (was {})", count(b.comments.len(), "comment"), a.comments.len()));
    }
    if a.revisions.len() != b.revisions.len() {
        said.push(format!("{} tracked", count(b.revisions.len(), "revision")));
    }
    for key in b.media.keys().filter(|k| !a.media.contains_key(*k)) {
        said.push(format!("added media `{key}`"));
    }
    let document_changed = !said.is_empty();
    if before.sel != s.sel {
        said.push(selection(s));
    }
    if !document_changed && let Some(l) = list(&field_diff("", &before.typing, &s.typing_props())) {
        said.push(format!("typing will use {l}"));
    }
    if said.is_empty() { "nothing changed".into() } else { said.join("; ") }
}

fn blocks(a: &Blocks, b: &Blocks, place: &str) -> Vec<String> {
    let same = |x: &Arc<Block>, y: &Arc<Block>| Arc::ptr_eq(x, y) || x == y;
    if a.len() == b.len() {
        return a.iter().zip(b).enumerate().filter(|(_, (x, y))| !same(x, y)).take(4).map(|(i, (x, y))| block_change(i, x, y, place)).collect();
    }
    let head = a.iter().zip(b).take_while(|(x, y)| same(x, y)).count();
    let tail = a.iter().rev().zip(b.iter().rev()).take(a.len().min(b.len()) - head).take_while(|(x, y)| same(x, y)).count();
    let added = b.get(head..b.len() - tail).unwrap_or_default();
    let removed = a.len() - tail - head;
    let gone = a.get(head..a.len() - tail).unwrap_or_default();
    let name = |blocks: &[Arc<Block>]| if kind(blocks) == "paragraph" { para_name(head, place) } else { block_name(head, place) };
    let mut said = match (removed, added) {
        (0, [one]) => return vec![format!("inserted {}", block_summary(head, one, place))],
        (0, _) => vec![format!("inserted {} at {}", count(added.len(), kind(added)), name(added))],
        (n, []) => vec![format!("removed {} at {}", count(n, kind(gone)), name(gone))],
        (n, _) => vec![format!("replaced {} at {} with {}", count(n, kind(gone)), name(gone), count(added.len(), kind(added)))],
    };
    said.extend(added.iter().enumerate().take(3).map(|(i, bl)| block_summary(head + i, bl, place)));
    said
}

fn kind(blocks: &[Arc<Block>]) -> &'static str {
    if blocks.iter().all(|b| matches!(**b, Block::Para(_))) {
        "paragraph"
    } else if blocks.iter().all(|b| matches!(**b, Block::Table(_))) {
        "table"
    } else {
        "block"
    }
}

fn block_change(i: usize, a: &Block, b: &Block, place: &str) -> String {
    match (a, b) {
        (Block::Para(x), Block::Para(y)) => para_change(i, x, y, place),
        (_, Block::Table(t)) => {
            format!("changed the table at {} (now {} × {})", block_name(i, place), count(t.rows.len(), "row"), count(t.cols(), "column"))
        }
        (_, Block::Para(p)) => format!("{} is now a paragraph reading {}", block_name(i, place), quote(&p.text)),
    }
}

fn para_change(i: usize, a: &Paragraph, b: &Paragraph, place: &str) -> String {
    let name = para_name(i, place);
    if a.text != b.text {
        let added: Vec<&InlineObject> = b.objects.iter().filter(|o| !a.objects.contains(o)).collect();
        return match (added.is_empty(), visible(&b.text).is_empty()) {
            (false, true) if !visible(&a.text).is_empty() => format!("replaced {} in {name} with {}", quote(&a.text), objects(added.into_iter())),
            (false, true) => format!("{name} now holds {}", objects(added.into_iter())),
            (false, false) => format!("added {} to {name}, which now reads {}", objects(added.into_iter()), quote(&b.text)),
            _ => format!("{name} now reads {}", quote(&b.text)),
        };
    }
    if a.objects.len() != b.objects.len() {
        return match b.objects.len().cmp(&a.objects.len()) {
            std::cmp::Ordering::Greater => format!("added {} to {name}", objects(b.objects.iter().filter(|o| !a.objects.contains(o)))),
            _ => format!("removed {} from {name}", objects(a.objects.iter().filter(|o| !b.objects.contains(o)))),
        };
    }
    if a.objects != b.objects {
        return format!("changed {} in {name}", objects(b.objects.iter().zip(&a.objects).filter(|(y, x)| x != y).map(|(y, _)| y)));
    }
    let mut what = run_diff(a, b);
    what.extend(field_diff("", &a.props, &b.props));
    if let Some(l) = list(&what) {
        return format!("formatted {name}: {l}");
    }
    if a.section != b.section {
        return match (&a.section, &b.section) {
            (_, Some(sec)) => {
                format!("section ending at {name}: {}", list(&field_diff("", &a.section.clone().unwrap_or_default(), sec)).unwrap_or_default())
            }
            _ => format!("removed the section break at {name}"),
        };
    }
    format!("changed {name}")
}

fn run_diff(a: &Paragraph, b: &Paragraph) -> Vec<String> {
    let props = |p: &Paragraph| -> Vec<(String, Value)> {
        let mut seen = vec![];
        for r in p.runs.iter().map(|r| &r.props).chain(std::iter::once(&p.mark)) {
            if let Some(m) = serde_json::to_value(r).ok().and_then(|v| v.as_object().cloned()) {
                for (k, v) in m {
                    if !seen.contains(&(k.clone(), v.clone())) {
                        seen.push((k, v));
                    }
                }
            }
        }
        seen
    };
    let (x, y) = (props(a), props(b));
    let mut keys: Vec<&String> = y.iter().chain(&x).map(|(k, _)| k).collect();
    keys.dedup();
    let mirrored = |k: &str| k.strip_suffix("Cs").is_some_and(|base| keys.iter().any(|key| *key == base));
    let mut out = vec![];
    for k in keys.iter().filter(|k| !mirrored(k)) {
        let before: Vec<&Value> = x.iter().filter(|(key, _)| key == *k).map(|(_, v)| v).collect();
        let after: Vec<&Value> = y.iter().filter(|(key, _)| key == *k).map(|(_, v)| v).collect();
        if before == after || out.iter().any(|o: &String| o.starts_with(&format!("{k} "))) {
            continue;
        }
        out.push(match after.as_slice() {
            [] => format!("{k} cleared"),
            vs => format!("{k} {}", vs.iter().map(|v| shown(v)).collect::<Vec<_>>().join("/")),
        });
    }
    out
}

fn parts(a: &Document, b: &Document) -> Vec<String> {
    let mut said = vec![];
    for (id, part) in &b.parts {
        match a.parts.get(id) {
            None => {
                let text = b.plain_text(StoryRef::Part(*id));
                said.push(if text.trim().is_empty() {
                    format!("added {}", part_name(b, *id))
                } else {
                    format!("added {} reading {}", part_name(b, *id), quote(text.trim_end()))
                });
            }
            Some(old) if old != part => said.extend(blocks(&old.blocks, &part.blocks, &part_name(b, *id))),
            Some(_) => {}
        }
    }
    for id in a.parts.keys().filter(|id| !b.parts.contains_key(id)) {
        said.push(format!("removed {}", part_name(a, *id)));
    }
    said
}

fn styles(a: &Document, b: &Document) -> Vec<String> {
    let mut said = vec![];
    if a.styles.default_chr != b.styles.default_chr || a.styles.default_para != b.styles.default_para {
        said.push("changed the document defaults".into());
    }
    for st in &b.styles.styles {
        match a.styles.styles.iter().find(|x| x.id == st.id) {
            None => said.push(format!("added the style `{}`", st.name)),
            Some(old) if old != st => said.push(format!("modified the style `{}`", st.name)),
            Some(_) => {}
        }
    }
    for st in a.styles.styles.iter().filter(|x| !b.styles.styles.iter().any(|y| y.id == x.id)) {
        said.push(format!("deleted the style `{}`", st.name));
    }
    said
}

fn field_diff<T: Serialize>(what: &str, a: &T, b: &T) -> Vec<String> {
    let map = |t: &T| serde_json::to_value(t).ok().and_then(|v| v.as_object().cloned()).unwrap_or_default();
    let (x, y) = (map(a), map(b));
    let mut out = vec![];
    let mirrored = |k: &str| k.strip_suffix("Cs").is_some_and(|base| y.contains_key(base) || x.contains_key(base));
    for (k, v) in y.iter().filter(|(k, _)| !mirrored(k)) {
        if x.get(k) != Some(v) {
            out.push(format!("{k} {}", shown(v)));
        }
    }
    for k in x.keys().filter(|k| !y.contains_key(*k)) {
        out.push(format!("{k} cleared"));
    }
    match (what, list(&out)) {
        (_, None) => vec![],
        ("", _) => out,
        (what, Some(l)) => vec![format!("{what}: {l}")],
    }
}

fn selection(s: &Session) -> String {
    let (a, b) = s.sel.ordered();
    let story = match a.story {
        StoryRef::Body => String::new(),
        StoryRef::Part(id) => format!(" of {}", part_name(&s.doc, id)),
    };
    if a == b {
        return format!("the caret is at {}{story}, offset {}", place(&a.path), a.off);
    }
    if a.path == b.path {
        let para = s.doc.para_at(&a);
        let text = para.and_then(|p| p.text.get(a.off..b.off)).unwrap_or_default();
        if let Some(o) = para.and_then(|p| p.object_at(a.off)).filter(|_| visible(text).is_empty()) {
            return format!("selected the {} in {}{story}, offsets {}–{}", object_name(o), place(&a.path), a.off, b.off);
        }
        return format!("selected {} in {}{story}, offsets {}–{}", quote(text), place(&a.path), a.off, b.off);
    }
    format!("selected from {}, offset {} to {}, offset {}{story}", place(&a.path), a.off, place(&b.path), b.off)
}

fn place(path: &Path) -> String {
    match path.cell() {
        Some((table, row, col)) if path.0.len() == 4 => {
            format!("paragraph {} of cell ({row}, {col}) of the table at block {}", path.last(), table.last())
        }
        Some(_) => format!("the paragraph at path {:?}", path.0),
        None => format!("paragraph {}", path.last()),
    }
}

fn block_name(i: usize, place: &str) -> String {
    if place.is_empty() { format!("block {i}") } else { format!("block {i} of {place}") }
}

fn para_name(i: usize, place: &str) -> String {
    if place.is_empty() { format!("paragraph {i}") } else { format!("paragraph {i} of {place}") }
}

fn block_summary(i: usize, b: &Block, place: &str) -> String {
    match b {
        Block::Para(p) if p.text.is_empty() => format!("{}, empty", para_name(i, place)),
        Block::Para(p) if visible(&p.text).is_empty() => format!("{} holding {}", para_name(i, place), objects(p.objects.iter())),
        Block::Para(p) => format!("{} reading {}", para_name(i, place), quote(&p.text)),
        Block::Table(t) => format!("a table of {} × {} at {}", count(t.rows.len(), "row"), count(t.cols(), "column"), block_name(i, place)),
    }
}

fn visible(text: &str) -> &str {
    text.trim_matches(wordcraft_doc::para::OBJ)
}

fn part_name(doc: &Document, id: u32) -> String {
    let kind = doc.parts.get(&id).and_then(|p| serde_json::to_value(p.kind).ok()).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    match kind.as_str() {
        "textBox" => format!("text box {id}"),
        k => format!("{k} {id}"),
    }
}

fn objects<'a>(it: impl Iterator<Item = &'a InlineObject>) -> String {
    let names: Vec<&str> = it.map(object_name).collect();
    match names.as_slice() {
        [] => "an object".into(),
        [one] => format!("a {one}"),
        many => format!("{} objects ({})", many.len(), many.join(", ")),
    }
}

fn object_name(o: &InlineObject) -> &'static str {
    match o {
        InlineObject::Image { .. } => "picture",
        InlineObject::Shape { .. } => "shape",
        InlineObject::Group { .. } => "group",
        InlineObject::Graphic { .. } => "graphic",
        InlineObject::Equation { .. } => "equation",
        InlineObject::Field { .. } | InlineObject::FieldStart { .. } => "field",
        InlineObject::NoteRef { .. } => "note reference",
        InlineObject::CommentStart { .. } | InlineObject::CommentEnd { .. } => "comment anchor",
        InlineObject::BookmarkStart { .. } | InlineObject::BookmarkEnd { .. } => "bookmark",
        InlineObject::ControlStart { .. } => "content control",
        InlineObject::Ruby { .. } => "phonetic guide",
        _ => "object",
    }
}

fn shown(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "none".into(),
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", f as i64),
            _ => n.to_string(),
        },
        Value::Object(m) if m.len() == 1 && m.contains_key("Rgb") => {
            match m["Rgb"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect::<Vec<_>>()) {
                Some(rgb) if rgb.len() == 3 => format!("{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]),
                _ => v.to_string(),
            }
        }
        other => {
            let s = other.to_string();
            if s.chars().count() > 80 { format!("{}…", s.chars().take(80).collect::<String>()) } else { s }
        }
    }
}

fn quote(text: &str) -> String {
    let text = text.replace(['\r', '\u{2029}'], "¶").replace(wordcraft_doc::para::OBJ, "[object]");
    match text.char_indices().nth(60) {
        Some((i, _)) => format!("`{}…`", text.get(..i).unwrap_or_default()),
        None if text.is_empty() => "nothing (it is empty)".into(),
        None => format!("`{text}`"),
    }
}

fn count(n: usize, what: &str) -> String {
    if n == 1 { format!("1 {what}") } else { format!("{n} {what}s") }
}

fn list(items: &[String]) -> Option<String> {
    if items.is_empty() { None } else { Some(items.join(", ")) }
}

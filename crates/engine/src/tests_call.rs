use serde_json::{Value, json};

use crate::{Session, cmd, params_doc};

fn s() -> Session {
    Session::new(wordcraft_doc::Document::new())
}

fn said(s: &mut Session, id: &str, p: Value) -> String {
    match s.call(id, &p) {
        Ok(Value::String(t)) => t,
        Ok(v) => v.to_string(),
        Err(e) => e,
    }
}

#[test]
fn every_params_doc_parses() {
    for c in cmd::registry().all() {
        let d = params_doc::parse(c.params);
        assert!(d.is_ok(), "{}: {:?}\n{}", c.id, d, c.params);
    }
}

#[test]
fn a_call_refuses_parameters_the_command_does_not_take() {
    let mut s = s();
    assert_eq!(
        said(&mut s, "text.insert", json!({"content": "Hi"})),
        "`text.insert` was given `content`, which it doesn't take. `text.insert` takes {\"text\": string, \"raw\"?: bool (no AutoCorrect or AutoFormat)}."
    );
    assert_eq!(
        said(&mut s, "edit.undo", json!({"steps": 2})),
        "`edit.undo` was given `steps`, which it doesn't take. `edit.undo` takes no parameters."
    );
    assert_eq!(
        said(&mut s, "format.size", json!([28])),
        "invalid parameters for `format.size`: the parameters are a JSON object, not an array. `format.size` takes {\"size\": number}."
    );
    assert_eq!(
        said(&mut s, "format.size", json!({})),
        "invalid parameters for `format.size`: `size` (number) is required. `format.size` takes {\"size\": number}."
    );
    assert_eq!(s.doc.plain_text(wordcraft_doc::StoryRef::Body), "");
    assert!(s.call("edit.advancedFind", &json!({"text": "x", "matchCase": true})).is_ok());
}

#[test]
fn an_unknown_command_names_the_commands_like_it() {
    let mut s = s();
    assert_eq!(
        said(&mut s, "new", json!({})),
        "`new` is not a command; commands named like it: file.new, text.newParagraph, view.newWindow, table.newStyle, review.newComment, file.newFromTemplate."
    );
    assert_eq!(said(&mut s, "file.nwe", json!({})), "`file.nwe` is not a command.");
}

#[test]
fn a_command_not_available_names_what_makes_it_so() {
    let mut s = s();
    assert_eq!(
        said(&mut s, "picture.size", json!({"width": 50})),
        "`picture.size` is not available right now: select a picture or shape first. insert.picture or insert.shape inserts one and selects it; select.objects {\"index\": n} selects an existing one."
    );
    assert_eq!(
        said(&mut s, "table.deleteRow", json!({})),
        "`table.deleteRow` is not available right now: the cursor isn't in a table. insert.table inserts one and puts the caret in its first cell; caret.set {\"pos\": {\"path\": [block, row, cell, paragraph], \"off\": 0}} moves into one."
    );
}

#[test]
fn a_call_that_answers_with_the_selection_says_what_changed() {
    let mut s = s();
    assert_eq!(
        said(&mut s, "text.insert", json!({"text": "Bondata Oy"})),
        "paragraph 0 now reads `Bondata Oy`; the caret is at paragraph 0, offset 10"
    );
    assert_eq!(said(&mut s, "select.paragraph", json!({})), "selected `Bondata Oy` in paragraph 0, offsets 0–10");
    assert_eq!(said(&mut s, "format.font", json!({"name": "Georgia"})), "formatted paragraph 0: font Georgia");
    assert_eq!(said(&mut s, "format.color", json!({"color": "0047BB"})), "formatted paragraph 0: color 0047BB");
    assert_eq!(said(&mut s, "para.align", json!({"value": "right"})), "formatted paragraph 0: align right");
    assert_eq!(said(&mut s, "caret.docEnd", json!({})), "the caret is at paragraph 0, offset 10");
    assert_eq!(said(&mut s, "format.bold", json!({})), "typing will use bold true");
    assert_eq!(said(&mut s, "text.newParagraph", json!({})), "inserted paragraph 1, empty; the caret is at paragraph 1, offset 0");
    assert_eq!(said(&mut s, "edit.undo", json!({})), "removed 1 paragraph at paragraph 1; the caret is at paragraph 0, offset 10");
    assert_eq!(said(&mut s, "insert.closeHeader", json!({})), "the caret is at paragraph 0, offset 0");
    assert_eq!(said(&mut s, "select.collapse", json!({})), "nothing changed");
    assert_eq!(said(&mut s, "styles.modify", json!({"style": "Normal", "chr": {"font": "Georgia"}})), "modified the style `Normal`");
    let r = s.call("layout.margins", &json!({"top": 56})).unwrap();
    assert_eq!(r["changed"], "page setup: marginTop 56");
    assert_eq!(r["marginTop"], 56.0);
}

#[test]
fn a_part_and_an_object_are_named_in_the_answer() {
    let mut s = s();
    let r = s.call("insert.header", &json!({"text": "Bondata"})).unwrap();
    assert_eq!(r["changed"], "added header 1 reading `Bondata`; the caret is at paragraph 0 of header 1, offset 0");
    assert_eq!(r["story"], 1);
    assert_eq!(said(&mut s, "caret.end", json!({})), "the caret is at paragraph 0 of header 1, offset 7");
    assert_eq!(
        said(&mut s, "text.insert", json!({"text": " Oy"})),
        "paragraph 0 of header 1 now reads `Bondata Oy`; the caret is at paragraph 0 of header 1, offset 10"
    );
    assert_eq!(said(&mut s, "insert.closeHeader", json!({})), "the caret is at paragraph 0, offset 0");
    let r = s.call("insert.table", &json!({"rows": 2, "cols": 3})).unwrap();
    assert_eq!(
        r["changed"],
        "inserted a table of 2 rows × 3 columns at block 0; the caret is at paragraph 0 of cell (0, 0) of the table at block 0, offset 0"
    );
    let mut t = Session::new(wordcraft_doc::Document::new());
    t.call("text.insert", &json!({"text": "Logo"})).unwrap();
    t.call("select.paragraph", &json!({})).unwrap();
    let r = said(&mut t, "insert.shape", json!({"kind": "rectangle"}));
    assert!(r.starts_with("replaced `Logo` in paragraph 0 with a shape; "), "{r}");
}

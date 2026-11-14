//! Change Case › Capitalize Each Word and Sentence case as in Word: words in capitals stay in
//! capitals when the text also has lowercase letters, a title word starts after any character that
//! is not a letter, digit or apostrophe, and a sentence ends only at `.`, `!` or `?` followed by
//! white space. Expected text measured in Microsoft 365 Word 16.0.20527 (Range.Case =
//! wdTitleWord / wdTitleSentence on the whole paragraph), 2026-11-14.

use serde_json::json;
use wordcraft_doc::StoryRef;

use crate::Session;

fn changed(text: &str, mode: &str) -> String {
    let mut s = Session::new(wordcraft_doc::Document::new());
    s.run("document.setText", &json!({ "text": text })).unwrap();
    s.run("select.all", &json!({})).unwrap();
    s.run("format.changeCase", &json!({ "mode": mode })).unwrap();
    s.doc.plain_text(StoryRef::Body)
}

/// (input, Capitalize Each Word, Sentence case) as Word gives them.
const WORD: &[(&str, &str, &str)] = &[
    ("abc HELLO wORLD. nasa e.g. done", "Abc HELLO World. Nasa E.G. Done", "Abc HELLO world. Nasa e.g. Done"),
    ("HELLO wORLD. tHE iPhone 3D and NASA", "HELLO World. The Iphone 3D And NASA", "HELLO world. The iphone 3D and NASA"),
    (
        "hello world. next one? yes! i'm o'neil-smith from mcdonald's, e.g. test",
        "Hello World. Next One? Yes! I'm O'neil-Smith From Mcdonald's, E.G. Test",
        "Hello world. Next one? Yes! I'm o'neil-smith from mcdonald's, e.g. Test",
    ),
    ("HELLO WORLD", "Hello World", "Hello world"),
    ("HELLO WORLD. NEXT ONE", "Hello World. Next One", "Hello world. Next one"),
    ("NASA rocks", "NASA Rocks", "NASA rocks"),
    ("nasa ROCKS. the end", "Nasa ROCKS. The End", "Nasa ROCKS. The end"),
    (
        "a-b x/y q_r a.b.c (x) [y] \"q\" ok,go o'neil",
        "A-B X/Y Q_R A.B.C (X) [Y] \"Q\" Ok,Go O'neil",
        "A-b x/y q_r a.b.c (x) [y] \"q\" ok,go o'neil",
    ),
    ("HELLO-world ABC-def X1 iPHONE", "HELLO-World ABC-Def X1 Iphone", "HELLO-world ABC-def X1 iphone"),
    ("word.next sentence. next", "Word.Next Sentence. Next", "Word.next sentence. Next"),
    ("end.  two spaces. \"quoted.\" then", "End.  Two Spaces. \"Quoted.\" Then", "End.  Two spaces. \"quoted.\" then"),
    ("i think so. i do", "I Think So. I Do", "I think so. I do"),
    ("A1B c2d", "A1B C2d", "A1B c2d"),
    ("  leading spaces. after (bracket) word", "  Leading Spaces. After (Bracket) Word", "  Leading spaces. After (bracket) word"),
];

#[test]
fn capitalize_each_word_as_word() {
    for (input, title, _) in WORD {
        assert_eq!(changed(input, "title"), *title, "title of {input:?}");
    }
}

#[test]
fn sentence_case_as_word() {
    for (input, _, sentence) in WORD {
        assert_eq!(changed(input, "sentence"), *sentence, "sentence case of {input:?}");
    }
}

#[test]
fn lower_upper_and_toggle_keep_no_words() {
    let t = "HELLO wORLD. tHE iPhone 3D and NASA";
    assert_eq!(changed(t, "lower"), "hello world. the iphone 3d and nasa");
    assert_eq!(changed(t, "upper"), "HELLO WORLD. THE IPHONE 3D AND NASA");
    assert_eq!(changed(t, "toggle"), "hello World. The IpHONE 3d AND nasa");
}

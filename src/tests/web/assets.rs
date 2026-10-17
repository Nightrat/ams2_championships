use super::*;

/// Every embedded script, by the name it is served under.
fn scripts() -> Vec<(&'static str, &'static str)> {
    vec![
        ("utils.js", JS_UTILS),
        ("telemetry.js", JS_TELEMETRY),
        ("track_map.js", JS_TRACK_MAP),
        ("live.js", JS_LIVE),
        ("career.js", JS_CAREER),
        ("manage.js", JS_MANAGE),
        ("contracts.js", JS_CONTRACTS),
        ("config.js", JS_CONFIG),
        ("saves.js", JS_SAVES),
        ("car_performance.js", JS_CARPERF),
        ("driver_performance.js", JS_DRIVERPERF),
        ("main.js", JS_MAIN),
    ]
}

/// What the scanner is in the middle of when it reaches a character.
#[derive(PartialEq)]
enum Mode {
    Code,
    Line,
    Block,
    Str(char),
    /// Inside a regex literal; the flag says whether a `[...]` class is open, since a `/` in
    /// one does not end the literal.
    Regex(bool),
}

/// A lexical once-over of one script: are its strings, regexes, comments and brackets all
/// closed where they have to be?
///
/// Not a parser — it cannot tell valid JavaScript from nonsense. It answers the one question
/// that matters here, which is whether the file will *load at all*: a browser drops a script
/// with a syntax error in silence, taking every function in it with it, and the tab it drives
/// then sits there rendering its static markup with nothing in the console to say why.
///
/// Returns the first complaint, with a line number.
fn scan(src: &str) -> Result<(), String> {
    let mut mode = Mode::Code;
    let mut depth: Vec<(char, usize)> = Vec::new();
    // The last character that can decide whether a `/` opens a regex or divides.
    let mut prev = ' ';
    let mut escaped = false;

    for (lineno, line) in src.lines().enumerate() {
        let line_no = lineno + 1;
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match mode {
                Mode::Line => break,
                Mode::Block => {
                    if c == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        mode = Mode::Code;
                    }
                }
                Mode::Str(q) => {
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == q {
                        mode = Mode::Code;
                    }
                }
                Mode::Regex(in_class) => {
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == '[' {
                        mode = Mode::Regex(true);
                    } else if c == ']' {
                        mode = Mode::Regex(false);
                    } else if c == '/' && !in_class {
                        mode = Mode::Code;
                        prev = '/';
                    }
                }
                Mode::Code => match c {
                    '/' if chars.peek() == Some(&'/') => mode = Mode::Line,
                    '/' if chars.peek() == Some(&'*') => mode = Mode::Block,
                    // A `/` divides after a value and starts a regex after anything else.
                    '/' if !(prev.is_alphanumeric() || "_$)]".contains(prev)) => {
                        mode = Mode::Regex(false)
                    }
                    '\'' | '"' | '`' => mode = Mode::Str(c),
                    '(' | '[' | '{' => depth.push((c, line_no)),
                    ')' | ']' | '}' => {
                        let want = match c {
                            ')' => '(',
                            ']' => '[',
                            _ => '{',
                        };
                        match depth.pop() {
                            Some((open, _)) if open == want => {}
                            Some((open, at)) => {
                                return Err(format!(
                                    "line {line_no}: found `{c}` closing the `{open}` opened on line {at}"
                                ))
                            }
                            None => return Err(format!("line {line_no}: stray `{c}`")),
                        }
                    }
                    _ => {}
                },
            }
            if !matches!(mode, Mode::Str(_) | Mode::Regex(_)) {
                escaped = false;
            }
            if !c.is_whitespace() && mode == Mode::Code {
                prev = c;
            }
        }
        // ES5 has no multi-line string, and a regex literal cannot span lines either. Both are
        // what a mangled escape produces, so both are worth catching here rather than in a
        // browser that will not say so.
        match mode {
            Mode::Str(q) => return Err(format!("line {line_no}: string opened with {q} never closes")),
            Mode::Regex(_) => {
                return Err(format!("line {line_no}: regex literal never closes"))
            }
            Mode::Line => mode = Mode::Code,
            _ => {}
        }
    }
    if let Some((open, at)) = depth.last() {
        return Err(format!("`{open}` opened on line {at} is never closed"));
    }
    Ok(())
}

/// The regression this exists for: `/["\]/g` leaves the character class open, so the regex
/// swallows the rest of the line and the file stops parsing. It cost a working Manage tab —
/// the page rendered, the tab was there, and every function in `manage.js` was simply absent.
#[test]
fn test_the_scanner_catches_a_mangled_escape() {
    let broken = "function f(id) {\n  return String(id).replace(/[\"\\]/g, '');\n}\n";
    let err = scan(broken).expect_err("an unterminated regex must be caught");
    assert!(err.contains("regex"), "{err}");
}

#[test]
fn test_the_scanner_accepts_the_things_it_must_not_trip_on() {
    // Division after a value, a regex after an operator, an escaped quote, an escaped slash,
    // a brace inside a string, and a URL inside a comment.
    let ok = r#"
var ratio = total / count;
var re = /^[a-z"\/]+$/g;
var s = 'it\'s {fine}';
// see https://example.com/foo
/* block { with } braces */
var o = { a: [1, 2], b: (3) };
"#;
    assert_eq!(scan(ok), Ok(()));
}

/// Guards every script the page serves, so a broken one cannot ship quietly.
#[test]
fn test_every_embedded_script_is_lexically_intact() {
    for (name, src) in scripts() {
        if let Err(e) = scan(src) {
            panic!("{name} would not load: {e}");
        }
    }
}

/// The two ids that carry the "no Custom AI folder" warning have to exist in the markup, because
/// both places that set them are `if (el)`-guarded — a renamed id would not throw, it would just
/// stop warning, and the whole point of the flag is that it appears for someone who has not gone
/// looking for it.
#[test]
fn test_the_missing_roster_folder_warning_is_wired_to_real_elements() {
    let html = build_base_html();
    for id in ["cfg-custom-ai-dir-missing", "tab-config-warn"] {
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "{id} is set by config.js but is not in the page"
        );
        assert!(
            JS_CONFIG.contains(&format!("getElementById('{id}')")),
            "{id} is in the page but nothing ever unhides it"
        );
    }
    // Hidden until the config says otherwise, so a set folder never flashes a warning on load.
    assert!(html.contains(r#"id="cfg-custom-ai-dir-missing" class="config-warn" hidden"#));
    assert!(html.contains(r#"id="tab-config-warn" class="tab-warn""#));
    // And the CSS that makes the "!" read as one, rather than as stray punctuation.
    assert!(CSS.contains(".tab-warn {"), "the badge has no styling");
    assert!(CSS.contains(".config-warn {"), "the notice has no styling");
}

/// The stylesheet with `/* … */` removed, so a rule's selector is not preceded by the comment
/// that introduces it.
fn strip_css_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(open) = rest.find("/*") {
        out.push_str(&rest[..open]);
        match rest[open + 2..].find("*/") {
            Some(close) => rest = &rest[open + 2 + close + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Every class on a `hidden` element that the stylesheet gives a `display` to, and whether it
/// opts back out for `[hidden]`.
///
/// Returns `(class, has_opt_out)` per class found, so the test can name the ones that fail.
fn hidden_classes_needing_an_opt_out() -> Vec<(String, bool)> {
    let html = build_base_html();
    // Comments go first: rules are found by splitting on `}`, which otherwise leaves the *next*
    // rule's leading comment glued to the front of its selector, and nothing ever matches.
    let css = strip_css_comments(CSS);
    let mut out = Vec::new();
    // Only the `hidden` *attribute* counts. A class merely spelled `…-hidden` is the other
    // mechanism — a class the script adds and removes — and it has no conflict to resolve.
    for tag in html.split('<').filter(|t| {
        t.split('>')
            .next()
            .is_some_and(|open| open.trim_end().ends_with(" hidden"))
    }) {
        let Some(classes) = tag.split("class=\"").nth(1).and_then(|c| c.split('"').next()) else {
            continue;
        };
        for class in classes.split_whitespace() {
            if out.iter().any(|(c, _): &(String, bool)| c == class) {
                continue;
            }
            // A `display` anywhere in a rule whose selector names this class on its own —
            // `.foo` or `.foo` inside a comma list, but not `.foo .bar` or `.foo[hidden]`.
            let sets_display = css.split('}').any(|rule| {
                let (selector, body) = rule.split_once('{').unwrap_or((rule, ""));
                selector.split(',').any(|s| s.trim() == format!(".{class}"))
                    && body.contains("display:")
            });
            if sets_display {
                out.push((class.to_string(), css.contains(&format!(".{class}[hidden]"))));
            }
        }
    }
    out
}

/// An author-level `display` outranks the UA stylesheet's `[hidden] { display: none }`, so an
/// element given both is **never hidden** — it just renders empty, which for a bordered warning
/// box is a stray strip and for a badge is a permanent alarm.
///
/// This has now bitten three times: the collapsed `<details>` on the lap charts, the live grid
/// warning, and the `!` on the Config tab. Scanning for it is cheaper than remembering it, and it
/// fails on the *next* one rather than on these.
#[test]
fn test_nothing_hidden_by_attribute_is_kept_visible_by_its_own_display_rule() {
    let found = hidden_classes_needing_an_opt_out();
    assert!(
        !found.is_empty(),
        "the scan found no hidden element with a display rule — it has stopped looking"
    );
    let missing: Vec<&str> = found
        .iter()
        .filter(|(_, ok)| !ok)
        .map(|(c, _)| c.as_str())
        .collect();
    assert!(
        missing.is_empty(),
        "these classes set `display` on an element that is hidden by attribute, so `hidden` \
         does nothing — add `.<class>[hidden] {{ display: none; }}`: {missing:?}"
    );
}

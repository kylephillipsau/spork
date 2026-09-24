//! The registers count themselves, because five times they did not.
//!
//! Question 150. The counts in [invariants.md], [open-questions.md] and the
//! README are all derivable from the tables printed directly beneath them, and
//! all of them have been typed by hand instead. Five have drifted:
//!
//! | | |
//! |---|---|
//! | the invariant total | found by D43, one short of its own sections |
//! | the question total | found by D51, understating by five |
//! | the `@projection` markers | found by D49 |
//! | the vacuity marks | found by D72, 53 against a table of 54 |
//! | "the deferred section holds 35" | found by D72 correcting the one above it |
//!
//! Every one was found by whoever next needed to touch the file, never by the
//! file itself — and the paragraph asserting the totals are derived rather than
//! carried forward was itself the fifth thing to drift. **A note asserting a
//! property is not a mechanism enforcing one**, which is the lesson D49 drew about
//! `@projection` and S5 drew the hard way.
//!
//! This is the mechanism. It parses the tables, counts them, and compares the
//! result to every number written about them — including the ones spelled out in
//! words, because the README is prose and should stay prose.
//!
//! Three-way, deliberately. The registers must agree with their own tables *and*
//! with the Rust that implements them, so an invariant added to the code without a
//! row, or a row without code, is caught by the same test that catches a stale
//! total.
//!
//! Needs no database: it reads files, so unlike the rest of the suite it can never
//! skip.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/invariants sits two below the root")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    let path = repo_root().join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// The cells of a Markdown table row, or None if the line is not one.
///
/// Separator rows (`|---|---|`) are not rows of anything and are dropped here so
/// no caller has to remember to.
fn cells(line: &str) -> Option<Vec<&str>> {
    let t = line.trim();
    if !t.starts_with('|') {
        return None;
    }
    let v: Vec<&str> = t.trim_matches('|').split('|').map(str::trim).collect();
    if v.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':') && !c.is_empty()) {
        return None;
    }
    Some(v)
}

fn numbers(s: &str) -> Vec<u32> {
    let mut out = vec![];
    let mut cur = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            cur.push(ch);
        } else if !cur.is_empty() {
            out.push(cur.parse().unwrap());
            cur.clear();
        }
    }
    if !cur.is_empty() {
        out.push(cur.parse().unwrap());
    }
    out
}

/// The value written in a two-column `| label | value |` row.
fn stated(doc: &str, label: &str) -> u32 {
    for line in doc.lines() {
        let Some(c) = cells(line) else { continue };
        if c.len() >= 2 && c[0].trim_matches('*').trim() == label {
            let n = numbers(c[1]);
            assert_eq!(n.len(), 1, "row {label:?} should carry exactly one number, got {n:?}");
            return n[0];
        }
    }
    panic!("no row labelled {label:?}");
}

/// English number words, so the README can keep its voice.
///
/// Bounded on purpose: this register counts things in the low hundreds, and a
/// parser that accepts "three million" would be inventing a requirement.
fn words_to_number(s: &str) -> u32 {
    const UNITS: &[(&str, u32)] = &[
        ("zero", 0), ("one", 1), ("two", 2), ("three", 3), ("four", 4), ("five", 5),
        ("six", 6), ("seven", 7), ("eight", 8), ("nine", 9), ("ten", 10),
        ("eleven", 11), ("twelve", 12), ("thirteen", 13), ("fourteen", 14),
        ("fifteen", 15), ("sixteen", 16), ("seventeen", 17), ("eighteen", 18),
        ("nineteen", 19), ("twenty", 20), ("thirty", 30), ("forty", 40),
        ("fifty", 50), ("sixty", 60), ("seventy", 70), ("eighty", 80), ("ninety", 90),
    ];
    let lowered = s.to_lowercase().replace('-', " ");
    let mut total = 0u32;
    let mut pending: Option<u32> = None;
    for word in lowered.split_whitespace() {
        match word {
            "and" => continue,
            "a" | "an" => pending = Some(1),
            "hundred" => {
                total += pending.unwrap_or(1) * 100;
                pending = None;
            }
            w => {
                let v = UNITS
                    .iter()
                    .find(|(name, _)| *name == w)
                    .unwrap_or_else(|| panic!("{s:?}: no number word {w:?}"))
                    .1;
                // "seventy four" is one number; "hundred eleven" is two terms.
                pending = match pending {
                    Some(p) if p >= 20 && p % 10 == 0 && v < 10 => Some(p + v),
                    Some(p) => {
                        total += p;
                        Some(v)
                    }
                    None => Some(v),
                };
            }
        }
    }
    total + pending.unwrap_or(0)
}

#[test]
fn number_words_parse() {
    for (words, want) in [
        ("seventy-four", 74),
        ("a hundred and eleven", 111),
        ("one hundred and eleven", 111),
        ("fifty-three", 53),
        ("fifty-six", 56),
        ("a hundred and ten", 110),
        ("two hundred", 200),
        ("nineteen", 19),
    ] {
        assert_eq!(words_to_number(words), want, "{words:?}");
    }
}

/// Every number written in words in the sentence containing `phrase`, in order.
///
/// "Fifty-six of a hundred and eleven assert an absence" states two, and reading
/// only the one nearest the phrase gets the wrong one — which this found the
/// first time it ran.
fn words_in_sentence(doc: &str, phrase: &str) -> Vec<u32> {
    let flat = doc.split_whitespace().collect::<Vec<_>>().join(" ");
    let at = flat
        .find(phrase)
        .unwrap_or_else(|| panic!("the phrase {phrase:?} is not in the document"));
    let start = flat[..at].rfind(". ").map(|i| i + 2).unwrap_or(0);
    let end = at + phrase.len();
    let mut out = vec![];
    let mut run: Vec<&str> = vec![];
    for w in flat[start..end].split_whitespace() {
        let word = w.trim_matches(|c: char| !c.is_alphabetic() && c != '-');
        let lower = word.to_lowercase();
        let is_num = lower == "hundred" || words_to_number_is_word(&lower);
        // "a" and "and" only continue a run, never start one.
        let continues = !run.is_empty() && (lower == "a" || lower == "and");
        if is_num || continues {
            run.push(word);
        } else if !run.is_empty() {
            out.push(words_to_number(&run.join(" ")));
            run.clear();
        }
    }
    if !run.is_empty() {
        out.push(words_to_number(&run.join(" ")));
    }
    out
}

/// The number written in words immediately before `phrase`.
fn stated_in_words(doc: &str, phrase: &str) -> u32 {
    let flat = doc.split_whitespace().collect::<Vec<_>>().join(" ");
    let at = flat
        .find(phrase)
        .unwrap_or_else(|| panic!("the phrase {phrase:?} is not in the document"));
    let before = &flat[..at];
    // Walk back over the words that could be part of a number.
    let mut taken: Vec<&str> = vec![];
    for w in before.split_whitespace().rev() {
        let word = w.trim_matches(|c: char| !c.is_alphabetic() && c != '-');
        let candidate = word.to_lowercase();
        let known = candidate == "and"
            || candidate == "a"
            || candidate == "hundred"
            || words_to_number_is_word(&candidate);
        if known {
            taken.push(word);
        } else {
            break;
        }
    }
    taken.reverse();
    assert!(!taken.is_empty(), "no number word before {phrase:?}");
    words_to_number(&taken.join(" "))
}

fn words_to_number_is_word(w: &str) -> bool {
    const ALL: &[&str] = &[
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
        "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen",
        "seventeen", "eighteen", "nineteen", "twenty", "thirty", "forty", "fifty",
        "sixty", "seventy", "eighty", "ninety",
    ];
    w.split('-').all(|part| ALL.contains(&part))
}

/// The invariant register against its own tables, and against the code.
#[test]
fn the_invariant_register_counts_itself() {
    let doc = read("docs/invariants.md");

    let (mut structural, mut job, mut vacuity) = (0u32, 0u32, 0u32);
    for line in doc.lines() {
        let Some(c) = cells(line) else { continue };
        let id = c[0];
        let is_s = id.starts_with('S') && id[1..].starts_with(|ch: char| ch.is_ascii_digit());
        let is_j = id.starts_with('J') && id[1..].starts_with(|ch: char| ch.is_ascii_digit());
        if !is_s && !is_j {
            continue;
        }
        if is_s {
            structural += 1;
        } else {
            job += 1;
        }
        if c.iter().any(|cell| cell.contains('●')) {
            vacuity += 1;
        }
    }

    assert_eq!(structural, stated(&doc, "Structural"), "structural rows against the stated count");
    assert_eq!(job, stated(&doc, "Job-asserted"), "job-asserted rows against the stated count");
    assert_eq!(
        structural + job,
        stated(&doc, "Total"),
        "the total against the rows that make it up"
    );
    // **`implemented` and `specified` are derived rather than read.** They were
    // typed, and `implemented` sat at 0 through the whole of the backend arriving
    // -- a hundred and two checks running, none of them counted, in the one file
    // whose opening paragraph calls that count "a more honest measure of progress
    // than the count of decisions". The register's status vocabulary makes them
    // complements: an entry is `specified` when nothing runs it, and `implemented`
    // once something does, which in the code is exactly `Check::Pending` against
    // everything else. Sixth number to drift, and the first to be caught before
    // somebody else needed the file.
    let implemented = spork_invariants::ALL
        .iter()
        .filter(|&&id| !matches!(spork_invariants::spec(id).check, spork_invariants::Check::Pending(_)))
        .count()
        + spork_invariants::jobs::ALL
            .iter()
            .filter(|&&id| {
                !matches!(
                    spork_invariants::jobs::spec(id).check,
                    spork_invariants::jobs::Check::Pending(_)
                )
            })
            .count();
    let specified = (structural + job) as usize - implemented;

    assert_eq!(
        implemented,
        stated(&doc, "`implemented`") as usize,
        "implemented against the checks that are not Pending"
    );
    assert_eq!(
        specified,
        stated(&doc, "`specified`") as usize,
        "specified against the entries with nothing behind them yet"
    );
    assert_eq!(vacuity, stated(&doc, "Marked for vacuity"), "vacuity marks against the stated count");

    // The prose says both of them again, in words.
    assert_eq!(
        words_in_sentence(&doc, "assert an absence and pass on an empty population"),
        vec![vacuity, structural + job],
        "the sentence spelling out the vacuity count and the total"
    );

    // And the code agrees with the register, which is the half a Markdown parser
    // cannot see: an invariant implemented without a row, or a row with nothing
    // behind it.
    assert_eq!(
        job as usize,
        spork_invariants::jobs::ALL.len(),
        "job-asserted rows against jobs::ALL"
    );
    assert_eq!(
        structural as usize,
        spork_invariants::ALL.len(),
        "structural rows against ALL"
    );
}

/// The question register against its own sections.
#[test]
fn the_question_register_counts_itself() {
    let doc = read("docs/open-questions.md");

    // Numbers retired in favour of another are aliases, not questions. The
    // duplicates table is the authority, so the rule is read rather than hardcoded.
    let mut retired: BTreeSet<u32> = BTreeSet::new();
    let mut in_duplicates = false;
    for line in doc.lines() {
        if line.starts_with("## ") {
            in_duplicates = line.starts_with("## Duplicates");
            continue;
        }
        if !in_duplicates {
            continue;
        }
        let Some(c) = cells(line) else { continue };
        let (number, kept) = (numbers(c[0]), c.get(2).copied().unwrap_or(""));
        if number.len() == 1 && numbers(kept) == number {
            continue; // kept itself: not an alias
        }
        if number.len() == 1 && !kept.contains("both") {
            retired.insert(number[0]);
        }
    }

    // Live sections, by heading.
    let mut section = String::new();
    let mut live: BTreeSet<u32> = BTreeSet::new();
    let mut settled: BTreeSet<u32> = BTreeSet::new();
    let mut in_minor_list = false;
    for line in doc.lines() {
        if let Some(h) = line.strip_prefix("## ") {
            section = h.trim().to_string();
            in_minor_list = false;
            continue;
        }
        if section.starts_with("Live") {
            // The minor section is a prose list, not a table, and it wraps. Taking
            // only the line carrying the colon lost eight of the fourteen, which
            // is how this was found.
            if section.starts_with("Live, minor") {
                if let Some((_, rest)) = line.split_once(':') {
                    in_minor_list = true;
                    for n in numbers(rest) {
                        live.insert(n);
                    }
                } else if in_minor_list {
                    if line.starts_with("---") || line.trim().is_empty() {
                        in_minor_list = false;
                    } else {
                        for n in numbers(line) {
                            live.insert(n);
                        }
                    }
                }
                continue;
            }
            let Some(c) = cells(line) else { continue };
            if c[0].starts_with(|ch: char| ch.is_ascii_digit()) {
                for n in numbers(c[0]) {
                    if !retired.contains(&n) {
                        live.insert(n);
                    }
                }
            }
        } else if section == "Settled" {
            let Some(c) = cells(line) else { continue };
            // Questions sit in the odd columns; the even ones name decisions.
            for (i, cell) in c.iter().enumerate() {
                if i % 2 == 0 {
                    for n in numbers(cell) {
                        if !retired.contains(&n) {
                            settled.insert(n);
                        }
                    }
                }
            }
        }
    }

    assert_eq!(
        live.len() as u32,
        stated(&doc, "Live total"),
        "live questions across the sections against the stated total"
    );
    assert_eq!(
        settled.len() as u32,
        stated(&doc, "Settled"),
        "settled questions in the table against the stated count"
    );

    // A question is open or it is answered. Being both means one of the two
    // tables was edited and the other was not, which is how a settled question
    // gets worked on twice.
    let both: Vec<u32> = live.intersection(&settled).copied().collect();
    assert!(both.is_empty(), "questions listed as both live and settled: {both:?}");

    // **A question answered in place is still counted as live**, which is the
    // door the checks above cannot see through: they read which section a row
    // sits in, never what the row says. Two questions were discharged by
    // striking the text through and writing "Discharged" into the cell, and the
    // register went on reporting fifty-seven live because both rows were still
    // under a Live heading. Every count agreed with every other count and all of
    // them were two too high — the same failure this file exists for, arriving
    // as prose rather than as a stale number.
    //
    // Answering a question means moving it to Settled. Saying so in the cell is
    // not moving it.
    let mut section = String::new();
    let mut answered_in_place: Vec<String> = vec![];
    for line in doc.lines() {
        if let Some(h) = line.strip_prefix("## ") {
            section = h.trim().to_string();
            continue;
        }
        if !section.starts_with("Live") {
            continue;
        }
        let Some(c) = cells(line) else { continue };
        if !c[0].starts_with(|ch: char| ch.is_ascii_digit()) {
            continue;
        }
        let body = c[1..].join(" ").to_lowercase();
        if body.contains("discharged") || body.contains("~~") || body.contains("**settled") {
            answered_in_place.push(c[0].to_string());
        }
    }
    assert!(
        answered_in_place.is_empty(),
        "questions answered in place but left under a Live heading, so they are still \
         counted as open: {answered_in_place:?} — move them to the Settled table"
    );
}

/// The outward-facing numbers, which are the ones a reader meets first.
///
/// **THE README NO LONGER STATES ANY OF THEM, DELIBERATELY.** It carried four
/// counts in English prose - decisions, rules, questions, vacuous rules - and
/// this test existed to keep them honest against the registers they were copied
/// from. The README is now what a reader needs to build and run the thing, and
/// a count of the author's own decisions is not that: it is a claim about
/// rigour aimed at somebody deciding whether to be impressed, which is the one
/// audience a README does not have.
///
/// So the four count assertions are gone rather than weakened, and what remains
/// is the decision RANGE in architecture.md, which is a real cross-reference
/// into the record and does drift. Deleting the whole test would have been the
/// easy read of this change and the wrong one.
#[test]
fn the_architecture_agrees_with_the_registers() {
    let architecture = read("docs/architecture.md");
    let invariants = read("docs/invariants.md");
    let questions = read("docs/open-questions.md");

    // **Every document, because the register is shared.** This read
    // `domain-model.md` alone, which was true while every decision lived there
    // and stopped being true the moment `interface-design.md` took D109 from
    // the same register. Twenty-three decisions were then invisible to the
    // check that exists to keep the outward numbers honest, and the README sat
    // on 108 while passing — the register's own guard drifting, which is the
    // failure this whole family of tests exists to catch.
    let highest_decision = fs::read_dir(repo_root().join("docs"))
        .expect("the docs directory")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .filter_map(|p| fs::read_to_string(p).ok())
        .flat_map(|doc| {
            doc.lines()
                .filter_map(|l| l.strip_prefix("### D"))
                .filter_map(|rest| {
                    let digits: String =
                        rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                    digits.parse::<u32>().ok()
                })
                .collect::<Vec<_>>()
        })
        .max()
        .expect("the record has decisions");

    // "D1 to DNN" names a range that keeps moving. One document states it now;
    // the README used to as well and no longer states any register figure.
    {
        let (name, doc) = ("docs/architecture.md", &architecture);
        let found = doc
            .split_whitespace()
            .collect::<Vec<_>>()
            .windows(4)
            .find_map(|w| {
                (w[0] == "D1" && w[1] == "to" && w[2].starts_with('D'))
                    .then(|| numbers(w[2]).first().copied())
                    .flatten()
            })
            .unwrap_or_else(|| panic!("{name} does not state the decision range"));
        assert_eq!(found, highest_decision, "{name} states the wrong end of the decision range");
    }

    // ---------------------------------------------------------------------
    // architecture.md quotes the same numbers, and nothing was reading it
    // ---------------------------------------------------------------------
    //
    // **Every assertion above is about the README, and the drift happened next
    // door.** While these four held, `architecture.md` sat on twenty-nine
    // migrations against sixty-six, a hundred and nine rules against a hundred
    // and twenty-six, sixty-nine implemented against a hundred and two, and
    // "what is not built is any application code: no server" against seven
    // thousand lines of one. It is the document that explains the system to
    // somebody new, so it is the worst of the two to have wrong.
    //
    // The migration count is the newest entry and the one that drifts fastest,
    // because every migration changes it. Derived from the directory, which is
    // the only authority there is.
    let migrations = fs::read_dir(repo_root().join("migrations"))
        .expect("the migration directory")
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .count() as u32;

    {
        let (name, doc) = ("docs/architecture.md", &architecture);
        assert_eq!(
            migrations,
            stated_in_words(doc, "migrations"),
            "{name}'s migration count against the migrations directory"
        );
    }

    assert_eq!(
        highest_decision,
        stated_in_words(&architecture, "recorded decisions"),
        "architecture.md's decision count against the highest D-heading"
    );
    assert_eq!(
        stated(&invariants, "Total"),
        stated_in_words(&architecture, "rules the design must always"),
        "architecture.md's rule count against the invariant register"
    );
    assert_eq!(
        stated(&invariants, "`implemented`"),
        stated_in_words(&architecture, "rules are implemented and none fails"),
        "architecture.md's implemented count against the invariant register"
    );
    assert_eq!(
        stated(&invariants, "Marked for vacuity"),
        stated_in_words(&architecture, "of the rules state that something does not happen"),
        "architecture.md's vacuity count against the invariant register"
    );
    assert_eq!(
        stated(&questions, "Live total"),
        stated_in_words(&architecture, "questions remain"),
        "architecture.md's question count against the question register"
    );
}

// Ported from openapi-sampler (src/samplers/string-regex.js), which is based on faker.js.
//
// Faker - Copyright (c) 2022-2023
// faker.js - Copyright (c) 2020 Marak Squires
// MIT License; the full notices are in LICENSE-MIT.
//
// The JavaScript original rewrites the pattern with a handful of regular
// expressions until none matches. The `regex` crate has no look-around, so each
// of those expressions is matched by hand below, keeping JavaScript's
// leftmost-match and backtracking behaviour.

use crate::utils::MAX_STRING_LENGTH;
use crate::Error;

/// Characters that end a line for JavaScript's `.`.
fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// A matched `{min}`, `{min,max}`, `?`, `*` or `+`.
#[derive(Debug, Clone, Copy)]
enum Quantifier<'p> {
    Symbol(char),
    Exact(&'p [char]),
    Range(&'p [char], &'p [char]),
}

fn digits_end(p: &[char], start: usize) -> usize {
    let mut i = start;
    while i < p.len() && p[i].is_ascii_digit() {
        i += 1;
    }
    i
}

/// Matches `\{(\d+)(?:,(\d+)|)\}` at `start`; returns the quantifier and its end.
fn braces_at(p: &[char], start: usize) -> Option<(Quantifier<'_>, usize)> {
    if p.get(start) != Some(&'{') {
        return None;
    }
    let min_end = digits_end(p, start + 1);
    if min_end == start + 1 {
        return None;
    }
    let min = &p[start + 1..min_end];
    if p.get(min_end) == Some(&',') {
        let max_end = digits_end(p, min_end + 1);
        if max_end > min_end + 1 && p.get(max_end) == Some(&'}') {
            return Some((
                Quantifier::Range(min, &p[min_end + 1..max_end]),
                max_end + 1,
            ));
        }
    }
    (p.get(min_end) == Some(&'}')).then_some((Quantifier::Exact(min), min_end + 1))
}

/// Matches `\{(\d+)(?:,(\d+)|)\}|(\?|\*|\+)` at `start`.
fn quantifier_at(p: &[char], start: usize) -> Option<(Quantifier<'_>, usize)> {
    braces_at(p, start).or_else(|| match p.get(start) {
        Some(&c) if matches!(c, '?' | '*' | '+') => Some((Quantifier::Symbol(c), start + 1)),
        _ => None,
    })
}

fn parse_count(digits: &[char]) -> f64 {
    digits
        .iter()
        .collect::<String>()
        .parse()
        .unwrap_or(f64::INFINITY)
}

/// `getRepetitionsBasedOnQuantifierParameters`, with faker's random choices
/// replaced by their lower bound (which is what openapi-sampler does).
fn repetitions(quantifier: Option<Quantifier<'_>>) -> f64 {
    match quantifier {
        Some(Quantifier::Symbol('+')) => 1.0,
        Some(Quantifier::Symbol(_)) => 0.0,
        Some(Quantifier::Exact(min)) | Some(Quantifier::Range(min, _)) => parse_count(min),
        None => 1.0,
    }
}

fn repeat(c: char, times: f64) -> Result<Vec<char>, Error> {
    if times > MAX_STRING_LENGTH {
        return Err(Error::TooLong);
    }
    Ok(vec![c; times as usize])
}

fn splice(p: &mut Vec<char>, start: usize, end: usize, with: Vec<char>) -> Result<(), Error> {
    if (p.len() - (end - start) + with.len()) as f64 > MAX_STRING_LENGTH {
        return Err(Error::TooLong);
    }
    p.splice(start..end, with);
    Ok(())
}

/// `(?![^[]*]|[^{]*})`: true when a `]` comes before any `[`, or a `}` before any `{`.
fn inside_class_or_braces(rest: &[char]) -> bool {
    let closes_before =
        |open: char, close: char| rest.iter().take_while(|&&c| c != open).any(|&c| c == close);
    closes_before('[', ']') || closes_before('{', '}')
}

/// `/([.A-Za-z0-9])(?:\{(\d+)(?:\,(\d+)|)\}|(\?|\*|\+))(?![^[]*]|[^{]*})/`
fn find_single_char(p: &[char]) -> Option<(usize, usize, char, Quantifier<'_>)> {
    (0..p.len()).find_map(|i| {
        let c = p[i];
        if !(c == '.' || c.is_ascii_alphanumeric()) {
            return None;
        }
        let (quantifier, end) = quantifier_at(p, i + 1)?;
        (!inside_class_or_braces(&p[end..])).then_some((i, end, c, quantifier))
    })
}

struct ClassMatch<'p> {
    start: usize,
    end: usize,
    negated: bool,
    dash: bool,
    content: &'p [char],
    quantifier: Option<Quantifier<'p>>,
}

/// `/\[(\^|)(-|)(.+?)\](?:\{(\d+)(?:\,(\d+)|)\}|(\?|\*|\+)|)/`
fn find_class(p: &[char]) -> Option<ClassMatch<'_>> {
    (0..p.len()).filter(|&i| p[i] == '[').find_map(|start| {
        let negations: &[bool] = if p.get(start + 1) == Some(&'^') {
            &[true, false]
        } else {
            &[false]
        };
        for &negated in negations {
            let after_negation = start + 1 + usize::from(negated);
            let dashes: &[bool] = if p.get(after_negation) == Some(&'-') {
                &[true, false]
            } else {
                &[false]
            };
            for &dash in dashes {
                let content_start = after_negation + usize::from(dash);
                // `(.+?)\]`: at least one character, then the first `]`.
                match p.get(content_start) {
                    Some(&c) if !is_line_terminator(c) => {}
                    _ => continue,
                }
                let mut k = content_start + 1;
                let close = loop {
                    match p.get(k) {
                        Some(']') => break Some(k),
                        Some(&c) if !is_line_terminator(c) => k += 1,
                        _ => break None,
                    }
                };
                let Some(close) = close else { continue };
                let (quantifier, end) = match quantifier_at(p, close + 1) {
                    Some((q, end)) => (Some(q), end),
                    None => (None, close + 1),
                };
                return Some(ClassMatch {
                    start,
                    end,
                    negated,
                    dash,
                    content: &p[content_start..close],
                    quantifier,
                });
            }
        }
        None
    })
}

fn is_special(c: char) -> bool {
    matches!(
        c,
        '-' | '!' | '@' | '#' | '$' | '&' | '(' | ')' | '`' | '.' | '+' | ',' | '/' | '"'
    )
}

/// `/(\d-\d|\w-\w|\d|\w|[-!@#$&()`.+,/"])/`: the leftmost match as a slice.
fn find_single_range(r: &[char]) -> Option<&[char]> {
    (0..r.len()).find_map(|i| {
        let c = r[i];
        let is_range = |test: fn(char) -> bool| {
            test(c) && r.get(i + 1) == Some(&'-') && r.get(i + 2).is_some_and(|&n| test(n))
        };
        if is_range(|c| c.is_ascii_digit()) || is_range(is_word) {
            Some(&r[i..i + 3])
        } else if is_word(c) || is_special(c) {
            Some(&r[i..i + 1])
        } else {
            None
        }
    })
}

fn class_codes(class: &ClassMatch<'_>) -> Result<Vec<u32>, Error> {
    let mut codes = Vec::new();
    if class.dash {
        codes.push(u32::from('-'));
    }
    let mut ranges = class.content;
    while let Some(range) = find_single_range(ranges) {
        if !range.contains(&'-') {
            codes.push(u32::from(range[0]));
        } else if range.len() == 3 {
            let (min, max) = (u32::from(range[0]), u32::from(range[2]));
            if min > max {
                return Err(Error::InvalidPattern(
                    "Character range provided is out of order.".to_owned(),
                ));
            }
            codes.extend(min..=max);
        }
        // A lone `-` adds nothing (the original computes a `NaN..NaN` range).

        // The original strips the match's length from the *start* of the ranges.
        ranges = &ranges[range.len()..];
    }
    if class.negated {
        for code in (48..=57).chain(65..=90).chain(97..=122) {
            match codes.iter().position(|&c| c == code) {
                Some(index) => {
                    codes.remove(index);
                }
                None => codes.push(code),
            }
        }
    }
    Ok(codes)
}

/// Generates a string for a regular expression `pattern`, the way
/// openapi-sampler does (always the first choice of every class and the lower
/// bound of every quantifier). Only a subset of regular-expression syntax is
/// understood; the rest is copied literally.
pub(crate) fn regex_sample(pattern: &str) -> Result<String, Error> {
    let mut p: Vec<char> = pattern.chars().collect();

    // pattern.replace(/^(\^)?(.*?)(\$)?$/, '$2'): `.` cannot cross line ends.
    if !p.iter().any(|&c| is_line_terminator(c)) {
        if p.first() == Some(&'^') {
            p.remove(0);
        }
        if p.last() == Some(&'$') {
            p.pop();
        }
    }

    while let Some((start, end, c, quantifier)) = find_single_char(&p) {
        let replacement = repeat(c, repetitions(Some(quantifier)))?;
        splice(&mut p, start, end, replacement)?;
    }

    while let Some(class) = find_class(&p) {
        let codes = class_codes(&class)?;
        let c = codes
            .first()
            .and_then(|&code| char::from_u32(code))
            .unwrap_or('\0');
        let replacement = repeat(c, repetitions(class.quantifier))?;
        let (start, end) = (class.start, class.end);
        splice(&mut p, start, end, replacement)?;
    }

    // `(.)\{(\d+)\,(\d+)\}` then `(.)\{(\d+)\}`
    for with_max in [true, false] {
        while let Some((start, end, c, min)) = find_repeat(&p, with_max)? {
            let replacement = repeat(c, min)?;
            splice(&mut p, start, end, replacement)?;
        }
    }

    Ok(p.into_iter().collect())
}

fn find_repeat(p: &[char], with_max: bool) -> Result<Option<(usize, usize, char, f64)>, Error> {
    for i in 0..p.len() {
        if is_line_terminator(p[i]) {
            continue;
        }
        match braces_at(p, i + 1) {
            Some((Quantifier::Range(min, max), end)) if with_max => {
                let (min, max) = (parse_count(min), parse_count(max));
                if min > max {
                    return Err(Error::InvalidPattern(
                        "Numbers out of order in {} quantifier.".to_owned(),
                    ));
                }
                return Ok(Some((i, end, p[i], min)));
            }
            Some((Quantifier::Exact(min), end)) if !with_max => {
                return Ok(Some((i, end, p[i], parse_count(min))));
            }
            _ => {}
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::regex_sample;

    #[test]
    fn faker_examples() {
        assert_eq!(regex_sample("#{5}").unwrap(), "#####");
        assert_eq!(regex_sample("#{2,9}").unwrap(), "##");
        assert_eq!(regex_sample("[1-7]").unwrap(), "1");
        assert_eq!(regex_sample("#{3}test[1-5]").unwrap(), "###test1");
        assert_eq!(regex_sample("[0-9a-dmno]").unwrap(), "0");
        assert_eq!(regex_sample("[^a-zA-Z0-8]").unwrap(), "9");
        assert_eq!(regex_sample("[a-d0-6]{2,8}").unwrap(), "aa");
        assert_eq!(regex_sample("[-a-z]{5}").unwrap(), "-----");
        assert_eq!(
            regex_sample("[A-Z0-9]{4}-[A-Z0-9]{4}").unwrap(),
            "AAAA-AAAA"
        );
        assert_eq!(regex_sample("Joh?n").unwrap(), "Jon");
        assert_eq!(regex_sample("ABC*DE").unwrap(), "ABDE");
        assert_eq!(regex_sample("bee+p").unwrap(), "beep");
        assert_eq!(regex_sample("^[0-9]$").unwrap(), "0");
        assert_eq!(regex_sample("[500-15000]").unwrap(), "5");
    }

    #[test]
    fn errors() {
        assert!(regex_sample("[z-a]").is_err());
        assert!(regex_sample("a{5,2}").is_ok()); // consumed as a single-char quantifier first
        assert!(regex_sample("#{5,2}").is_err());
    }

    #[test]
    fn quantifiers_inside_classes_are_left_for_the_class_pass() {
        // `a+` sits inside `[...]`, so the single-character pass skips it.
        assert_eq!(regex_sample("[a+]").unwrap(), "a");
    }
}

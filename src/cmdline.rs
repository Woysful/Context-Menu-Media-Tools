//! The commands of presets.toml are complete ffmpeg command lines with placeholders:
//!
//!     ffmpeg -n -i {input} {vf} -c:v libx264 -crf {q} {output}
//!
//! The line is split into words FIRST (quotes work like in a shell) and the placeholders are
//! replaced afterwards, so a file name with spaces or quotes can never break the command.
//! A word that is exactly `{name}` may expand to several arguments (or to none).

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    S(String),
    L(Vec<String>),
}

impl Val {
    pub fn s(v: impl Into<String>) -> Val {
        Val::S(v.into())
    }
    pub fn l<I, T>(v: I) -> Val
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        Val::L(v.into_iter().map(Into::into).collect())
    }
}

/// Split into words: whitespace separates, "double quotes" (with \" and \\ inside) and
/// 'single quotes' (literal) group, a backslash outside quotes escapes the next character.
pub fn split(line: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        None => return Err("unterminated \" in the command".into()),
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(n @ ('"' | '\\')) => cur.push(n),
                            Some(n) => {
                                cur.push('\\');
                                cur.push(n);
                            }
                            None => return Err("unterminated \" in the command".into()),
                        },
                        Some(n) => cur.push(n),
                    }
                }
            }
            '\'' => {
                in_word = true;
                loop {
                    match chars.next() {
                        None => return Err("unterminated ' in the command".into()),
                        Some('\'') => break,
                        Some(n) => cur.push(n),
                    }
                }
            }
            '\\' => {
                in_word = true;
                match chars.next() {
                    Some(n) => cur.push(n),
                    None => cur.push('\\'),
                }
            }
            c => {
                in_word = true;
                cur.push(c);
            }
        }
    }
    if in_word {
        words.push(cur);
    }
    Ok(words)
}

/// The pieces of a word: literal text and `{name}` placeholders (lower-case letters and `_`).
/// Braces that do not form a placeholder (`{1,2}`) are literal text.
fn pieces(word: &str) -> Vec<(bool, &str)> {
    let b = word.as_bytes();
    let mut out = Vec::new();
    let (mut literal, mut i) = (0, 0);
    while i < b.len() {
        if b[i] == b'{' {
            let len = b[i + 1..].iter().take_while(|c| c.is_ascii_lowercase() || **c == b'_').count();
            if len > 0 && b.get(i + 1 + len) == Some(&b'}') {
                if i > literal {
                    out.push((false, &word[literal..i]));
                }
                out.push((true, &word[i + 1..i + 1 + len]));
                i += len + 2;
                literal = i;
                continue;
            }
        }
        i += 1;
    }
    if literal < word.len() {
        out.push((false, &word[literal..]));
    }
    out
}

/// Placeholders used by a command (for the checks of the config)
pub fn placeholders(line: &str) -> Result<Vec<String>, String> {
    Ok(split(line)?.iter().flat_map(|w| pieces(w).into_iter().filter(|(p, _)| *p).map(|(_, n)| n.to_string()).collect::<Vec<_>>()).collect())
}

/// Split the line and replace the placeholders in one pass, so the text of a value (a file
/// name with braces in it) is never looked at again. An unknown `{name}` is an error.
pub fn expand(line: &str, vars: &[(&str, Val)]) -> Result<Vec<String>, String> {
    let lookup = |name: &str| vars.iter().find(|(n, _)| *n == name).map(|(_, v)| v).ok_or_else(|| format!("unknown placeholder {{{name}}} in the command"));
    let mut out = Vec::new();
    for word in split(line)? {
        let parts = pieces(&word);
        // a whole word `{name}`: may be several arguments, or none
        if let [(true, name)] = parts.as_slice() {
            match lookup(name)? {
                Val::L(list) => out.extend(list.iter().cloned()),
                Val::S(s) => out.push(s.clone()),
            }
            continue;
        }
        let mut w = String::new();
        for (is_placeholder, text) in parts {
            if !is_placeholder {
                w.push_str(text);
                continue;
            }
            match lookup(text)? {
                Val::S(s) => w.push_str(s),
                Val::L(l) => w.push_str(&l.join(" ")),
            }
        }
        out.push(w);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_like_a_shell() {
        assert_eq!(split("ffmpeg -i  a b").unwrap(), ["ffmpeg", "-i", "a", "b"]);
        assert_eq!(split(r#"a "b c" 'd e' f\ g"#).unwrap(), ["a", "b c", "d e", "f g"]);
        assert_eq!(split(r#"-vf "scale=w='if(gt(iw,ih),-2,720)':h=-2""#).unwrap(), ["-vf", "scale=w='if(gt(iw,ih),-2,720)':h=-2"]);
        assert_eq!(split(r#"x "" y"#).unwrap(), ["x", "", "y"]);
        assert_eq!(split(r#""a\"b""#).unwrap(), ["a\"b"]);
        assert!(split("a \"b").is_err());
        assert!(split("a 'b").is_err());
        assert!(split("").unwrap().is_empty());
    }

    #[test]
    fn expands_lists_and_inline_values() {
        let vars = [("input", Val::s("/my dir/a b.mp4")), ("vf", Val::l(["-vf", "scale=1:1"])), ("none", Val::L(vec![])), ("q", Val::s("22"))];
        let out = expand("ffmpeg -n -i {input} {vf} {none} -crf {q} -x264-params q={q}:z=1 {input}", &vars).unwrap();
        assert_eq!(out, ["ffmpeg", "-n", "-i", "/my dir/a b.mp4", "-vf", "scale=1:1", "-crf", "22", "-x264-params", "q=22:z=1", "/my dir/a b.mp4"]);
    }

    #[test]
    fn filenames_are_not_re_parsed() {
        let vars = [("input", Val::s("it's \"quoted\" {q}.mp4")), ("q", Val::s("1"))];
        let out = expand("ffmpeg -i {input}", &vars).unwrap();
        assert_eq!(out[2], "it's \"quoted\" {q}.mp4");
    }

    #[test]
    fn values_are_not_expanded_again() {
        // the file name holds braces: inside a longer word it must stay as it is
        let vars = [("input", Val::s("/a/{q} {x}.mp4")), ("q", Val::s("1"))];
        let out = expand("ffmpeg -metadata title={input} -i {input} -crf {q}", &vars).unwrap();
        assert_eq!(out, ["ffmpeg", "-metadata", "title=/a/{q} {x}.mp4", "-i", "/a/{q} {x}.mp4", "-crf", "1"]);
        assert_eq!(expand("a{q}b{q}", &vars).unwrap(), ["a1b1"]);
    }

    #[test]
    fn unknown_placeholder_is_an_error() {
        assert!(expand("ffmpeg -i {inptu}", &[("input", Val::s("a"))]).is_err());
        // braces that are not placeholders stay
        assert_eq!(expand("ffmpeg -vf {1,2}", &[]).unwrap()[2], "{1,2}");
        assert_eq!(placeholders("ffmpeg -i {input} -vf \"{scale}x\" {output}").unwrap(), ["input", "scale", "output"]);
    }
}

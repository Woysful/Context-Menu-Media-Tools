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

fn placeholder_names(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '{' {
            let mut j = i + 1;
            while j < b.len() && (b[j].is_ascii_lowercase() || b[j] == '_') {
                j += 1;
            }
            if j > i + 1 && j < b.len() && b[j] == '}' {
                out.push(b[i + 1..j].iter().collect());
                i = j;
            }
        }
        i += 1;
    }
    out
}

/// Placeholders used by a command (for the checks of the config)
pub fn placeholders(line: &str) -> Result<Vec<String>, String> {
    Ok(split(line)?.iter().flat_map(|w| placeholder_names(w)).collect())
}

/// Split the line and replace the placeholders. An unknown `{name}` is an error.
pub fn expand(line: &str, vars: &[(&str, Val)]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for word in split(line)? {
        // a whole word `{name}`: may be several arguments
        if let Some(name) = word.strip_prefix('{').and_then(|w| w.strip_suffix('}')) {
            if let Some((_, v)) = vars.iter().find(|(n, _)| *n == name) {
                match v {
                    Val::L(list) => out.extend(list.iter().cloned()),
                    Val::S(s) => out.push(s.clone()),
                }
                continue;
            }
        }
        let mut w = word;
        for (name, v) in vars {
            let key = format!("{{{name}}}");
            if w.contains(&key) {
                let text = match v {
                    Val::S(s) => s.clone(),
                    Val::L(l) => l.join(" "),
                };
                w = w.replace(&key, &text);
            }
        }
        if let Some(left) = placeholder_names(&w).first() {
            return Err(format!("unknown placeholder {{{left}}} in the command"));
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
        let vars = [
            ("input", Val::s("/my dir/a b.mp4")),
            ("vf", Val::l(["-vf", "scale=1:1"])),
            ("none", Val::L(vec![])),
            ("q", Val::s("22")),
        ];
        let out = expand("ffmpeg -n -i {input} {vf} {none} -crf {q} -x264-params q={q}:z=1 {input}", &vars).unwrap();
        assert_eq!(
            out,
            ["ffmpeg", "-n", "-i", "/my dir/a b.mp4", "-vf", "scale=1:1", "-crf", "22", "-x264-params", "q=22:z=1", "/my dir/a b.mp4"]
        );
    }

    #[test]
    fn filenames_are_not_re_parsed() {
        let vars = [("input", Val::s("it's \"quoted\" {q}.mp4")), ("q", Val::s("1"))];
        let out = expand("ffmpeg -i {input}", &vars).unwrap();
        assert_eq!(out[2], "it's \"quoted\" {q}.mp4");
    }

    #[test]
    fn unknown_placeholder_is_an_error() {
        assert!(expand("ffmpeg -i {inptu}", &[("input", Val::s("a"))]).is_err());
        // braces that are not placeholders stay
        assert_eq!(expand("ffmpeg -vf {1,2}", &[]).unwrap()[2], "{1,2}");
        assert_eq!(placeholders("ffmpeg -i {input} -vf \"{scale}x\" {output}").unwrap(), ["input", "scale", "output"]);
    }
}

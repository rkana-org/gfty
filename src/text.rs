use anyhow::{Result, bail};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRun {
    pub filament: u32,
    pub text: String,
}

/// Parse nested color markup. Plain text starts at filament 1; `{}`, `[]`, and
/// `<>` select 2, 3, and 4, while `!N{}` selects any filament. Backslash escapes
/// the next character.
pub fn parse_colored_text(input: &str) -> Result<Vec<TextRun>> {
    let chars: Vec<char> = input.chars().collect();
    let mut parser = Parser {
        chars: &chars,
        offset: 0,
        runs: Vec::new(),
    };
    parser.parse_region(1, None)?;
    Ok(parser.runs)
}

struct Parser<'a> {
    chars: &'a [char],
    offset: usize,
    runs: Vec<TextRun>,
}

impl Parser<'_> {
    fn parse_region(&mut self, filament: u32, expected_close: Option<char>) -> Result<()> {
        let mut plain = String::new();

        while self.offset < self.chars.len() {
            let c = self.chars[self.offset];

            if c == '\\' {
                self.offset += 1;
                let Some(&escaped) = self.chars.get(self.offset) else {
                    bail!("trailing escape at character {}", self.offset);
                };
                plain.push(escaped);
                self.offset += 1;
                continue;
            }

            if matches!(c, '}' | ']' | '>') {
                if expected_close == Some(c) {
                    self.flush(filament, &mut plain);
                    self.offset += 1;
                    return Ok(());
                }
                bail!(
                    "unexpected closing delimiter {c:?} at character {}",
                    self.offset
                );
            }

            if let Some((child_filament, close)) = shorthand(c) {
                self.flush(filament, &mut plain);
                self.offset += 1;
                self.parse_region(child_filament, Some(close))?;
                continue;
            }

            if c == '!'
                && let Some((child_filament, after_open)) = self.explicit_color()?
            {
                self.flush(filament, &mut plain);
                self.offset = after_open;
                self.parse_region(child_filament, Some('}'))?;
                continue;
            }

            plain.push(c);
            self.offset += 1;
        }

        self.flush(filament, &mut plain);
        if let Some(close) = expected_close {
            bail!("missing closing delimiter {close:?} at end of text");
        }
        Ok(())
    }

    fn explicit_color(&self) -> Result<Option<(u32, usize)>> {
        let mut cursor = self.offset + 1;
        let digits_start = cursor;
        while self.chars.get(cursor).is_some_and(|c| c.is_ascii_digit()) {
            cursor += 1;
        }
        if cursor == digits_start || self.chars.get(cursor) != Some(&'{') {
            return Ok(None);
        }
        let digits: String = self.chars[digits_start..cursor].iter().collect();
        let filament = digits.parse::<u32>().map_err(|_| {
            anyhow::anyhow!("filament index is too large at character {}", self.offset)
        })?;
        Ok(Some((filament, cursor + 1)))
    }

    fn flush(&mut self, filament: u32, text: &mut String) {
        if text.is_empty() {
            return;
        }
        if let Some(last) = self.runs.last_mut()
            && last.filament == filament
        {
            last.text.push_str(text);
        } else {
            self.runs.push(TextRun {
                filament,
                text: std::mem::take(text),
            });
            return;
        }
        text.clear();
    }
}

fn shorthand(open: char) -> Option<(u32, char)> {
    match open {
        '{' => Some((2, '}')),
        '[' => Some((3, ']')),
        '<' => Some((4, '>')),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs(input: &str) -> Vec<(u32, String)> {
        parse_colored_text(input)
            .unwrap()
            .into_iter()
            .map(|run| (run.filament, run.text))
            .collect()
    }

    #[test]
    fn parses_shorthand_and_restores_parent() {
        assert_eq!(
            runs("M{3}x[10]"),
            vec![
                (1, "M".into()),
                (2, "3".into()),
                (1, "x".into()),
                (3, "10".into())
            ]
        );
    }

    #[test]
    fn supports_nested_explicit_colors() {
        assert_eq!(
            runs("!2{Green !0{Normal} Green}"),
            vec![
                (2, "Green ".into()),
                (0, "Normal".into()),
                (2, " Green".into())
            ]
        );
    }

    #[test]
    fn supports_nested_shorthand() {
        assert_eq!(
            runs("{one [two] one}"),
            vec![(2, "one ".into()), (3, "two".into()), (2, " one".into())]
        );
    }

    #[test]
    fn backslash_escapes_markup() {
        assert_eq!(runs(r"\{x\} \!2\{y\} \\"), vec![(1, "{x} !2{y} \\".into())]);
    }

    #[test]
    fn rejects_mismatched_or_unclosed_markup() {
        assert!(parse_colored_text("{abc]").is_err());
        assert!(parse_colored_text("{abc").is_err());
        assert!(parse_colored_text("abc}").is_err());
        assert!(parse_colored_text("abc\\").is_err());
    }
}

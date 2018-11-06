const ESCAPED: u8 = 1;
const INNER_TOKENS: u8 = 2;

/// Simple, efficient shell-like string tokenizer, and expander extraordinaire.
#[derive(Debug, Clone)]
pub struct Tokenizer<'a> {
    data: &'a str,
    read: usize,
    flags: u8,
    level: u8,
    escape: u8,
}

/// An individual token, which may be a variable key, an escaped character, or plain text.
#[derive(Debug, PartialEq)]
pub enum Token<'a> {
    /// The character that follows the escape byte.
    Escaped(char),
    /// The discovered key, and an indication of whether further tokenization is possible.
    Key(&'a str, bool),
    /// Text which did not contain any matched patterns.
    Normal(&'a str),
}

impl<'a> Tokenizer<'a> {
    /// Constructs a new tokenizer, which uses `\` as the default escape character.
    ///
    /// ```rust
    /// use token_expander::{Token, Tokenizer, TokenizerExt};
    ///
    /// assert_eq!(
    ///     Tokenizer::new("foo###${bar}")
    ///         .set_escape(b'#')
    ///         .collect::<Vec<_>>(),
    ///     vec![
    ///         Token::Normal("foo"),
    ///         Token::Escaped('#'),
    ///         Token::Escaped('$'),
    ///         Token::Normal("{bar}"),
    ///     ]
    /// );
    /// ```
    pub fn new(data: &'a str) -> Tokenizer<'a> {
        Tokenizer { data, read: 0, level: 0, flags: 0, escape: b'\\' }
    }

    fn escaped_character(&mut self) -> Token<'a> {
        match self.data[self.read..].chars().next() {
            Some(char) => {
                self.read += char.len_utf8();
                Token::Escaped(char)
            }
            None => Token::Escaped('\\'),
        }
    }
}

/// Trait for providing expansion abstractions to any type which implements it.
pub trait TokenizerExt<'a>: Iterator<Item = Token<'a>> {
    /// Retrieve the escape char being used by the tokenizer.
    fn get_escape(&self) -> u8;

    /// Define a new escape character to use instead of `\`.
    fn set_escape(self, escape: u8) -> Self;

    /// Whether the inner string is empty or not.
    fn is_empty(&self) -> bool {
        self.len() != 0
    }

    /// The length of the inner string.
    fn len(&self) -> usize;

    /// The number of bytes that have been read from the inner string.
    fn read(&self) -> usize;

    /// Abstraction for handling consumption and expansion of tokens.
    ///
    /// # Notes
    ///
    /// If the `map` closure returns `Ok(false)` or `Err(why)`, expansion will stop early.
    ///
    /// # Example
    ///
    /// ```rust
    /// use token_expander::{Token, Tokenizer, TokenizerExt};
    ///
    /// let url = "https://${domain}/${repo}/${name}/${name}_${version}_${arch}.deb";
    /// assert_eq!(
    ///     Tokenizer::new(url).expand(|buf, key| {
    ///         match key {
    ///             Token::Normal(text)       => buf.push_str(text),
    ///             Token::Key("name", _)     => buf.push_str("system76"),
    ///             Token::Key("version", _)  => buf.push_str("1.0.0"),
    ///             Token::Key("arch", _)     => buf.push_str("amd64"),
    ///             Token::Key("domain", _)   => buf.push_str("apt.pop-os.org"),
    ///             Token::Key("repo", _)     => buf.push_str("free"),
    ///             Token::Key(other, _)      => return Err(format!("unsupported key: {}", other)),
    ///             Token::Escaped('n')       => buf.push('\n'),
    ///             Token::Escaped('t')       => buf.push('\t'),
    ///             Token::Escaped(character) => buf.push(character),
    ///         }
    ///         Ok(true)
    ///     }),
    ///     Ok("https://apt.pop-os.org/free/system76/system76_1.0.0_amd64.deb".into())
    /// );
    /// ```
    fn expand<'b, T, F>(&mut self, mut map: F) -> Result<String, T>
        where F: FnMut(&mut String, Token) -> Result<bool, T>,
    {
        let mut output = String::with_capacity(self.len() * 2);
        for token in self {
            if ! map(&mut output, token)? {
                break;
            }
        }

        output.shrink_to_fit();
        Ok(output)
    }
}

impl<'a> TokenizerExt<'a> for Tokenizer<'a> {
    fn get_escape(&self) -> u8 {
        self.escape
    }

    fn set_escape(mut self, escape: u8) -> Self {
        self.escape = escape;
        self
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn read(&self) -> usize {
        self.read
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Token<'a>> {
        if self.flags & ESCAPED != 0 {
            self.flags ^= ESCAPED;
            return Some(self.escaped_character());
        }

        if self.read >= self.data.len() {
            return None;
        }

        let mut start = self.read;
        let bytes = self.data.as_bytes();
        while self.read < self.data.len() {
            if self.level == 0 && bytes[self.read] == self.escape {
                let token = &self.data[start..self.read];
                self.read += 1;

                if token.is_empty() {
                    return Some(self.escaped_character());
                } else {
                    self.flags |= ESCAPED;
                    return Some(Token::Normal(token));
                }
            } else if self.level != 0 && bytes[self.read] == b'}' {
                self.read += 1;
                self.level -= 1;
                if self.level == 0 {
                    let inner_tokens = self.flags & INNER_TOKENS != 0;
                    self.flags &= INNER_TOKENS ^ 255;
                    let token = Token::Key(&self.data[start..self.read - 1], inner_tokens);
                    return Some(token);
                }
            } else if self.data.len() - self.read > 2 && &bytes[self.read..self.read + 2][..] == b"${" {
                if self.level == 0 {
                    let token = &self.data[start..self.read];
                    self.level += 1;
                    self.read += 2;
                    if !token.is_empty() {
                        return Some(Token::Normal(token));
                    } else {
                        start = self.read;
                    }
                } else {
                    self.flags |= INNER_TOKENS;
                    self.level += 1;
                    self.read += 2;
                }
            } else {
                self.read += 1;
            }
        }

        self.read = self.data.len();
        let remaining = &self.data[start..];
        if remaining.is_empty() {
            None
        } else if self.level != 0 {
            let inner_tokens = self.flags & INNER_TOKENS != 0;
            self.flags &= INNER_TOKENS ^ 255;
            Some(Token::Key(&remaining, inner_tokens))
        } else {
            Some(Token::Normal(remaining))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens() {
        let url = "https://${domain}/${repo}/${name}/${name}_${version}_${arch}.deb";
        assert_eq!(
            Tokenizer::new(url).collect::<Vec<_>>(),
            vec![
                Token::Normal("https://"),
                Token::Key("domain", false),
                Token::Normal("/"),
                Token::Key("repo", false),
                Token::Normal("/"),
                Token::Key("name", false),
                Token::Normal("/"),
                Token::Key("name", false),
                Token::Normal("_"),
                Token::Key("version", false),
                Token::Normal("_"),
                Token::Key("arch", false),
                Token::Normal(".deb"),
            ]
        );
    }

    #[test]
    fn nested_tokens() {
        let sample = "${foo ${bar}} ${${foo} bar}";
        assert_eq!(
            Tokenizer::new(sample).collect::<Vec<_>>(),
            vec![
                Token::Key("foo ${bar}", true),
                Token::Normal(" "),
                Token::Key("${foo} bar", true)
            ]
        );
    }

    #[test]
    fn expander() {
        let url = "https://app.domain.org/${name}/${name}_${version}.deb";
        assert_eq!(
            Tokenizer::new(url).expand(|buf, key| {
                match key {
                    Token::Normal(text) => buf.push_str(text),
                    Token::Key("name", _) => buf.push_str("system76"),
                    Token::Key("version", _) => buf.push_str("1.0.0"),
                    Token::Key(other, _) => return Err(format!("unsupported key: {}", other)),
                    Token::Escaped(_) => panic!("didn't expect an escaped character"),
                }

                Ok(true)
            }),
            Ok("https://app.domain.org/system76/system76_1.0.0.deb".into())
        );

        assert_eq!(
            Tokenizer::new("https://app.domain.org/package_version.deb")
                .expand(|buf, key| -> Result<bool, String> {
                    match key {
                        Token::Normal(text) => buf.push_str(text),
                        Token::Key(key, _) if key == "foo" => {
                            buf.push_str("bar");
                        }
                        _ => (),
                    }

                    Ok(true)
                }
            ),
            Ok("https://app.domain.org/package_version.deb".into())
        )
    }

    #[test]
    fn escaper() {
        let pattern = "foo\\${bar}";
        assert_eq!(
            Tokenizer::new(pattern).collect::<Vec<_>>(),
            vec![
                Token::Normal("foo"),
                Token::Escaped('$'),
                Token::Normal("{bar}"),
            ]
        );

        assert_eq!(
            Tokenizer::new("foo###${bar}").set_escape(b'#').collect::<Vec<_>>(),
            vec![
                Token::Normal("foo"),
                Token::Escaped('#'),
                Token::Escaped('$'),
                Token::Normal("{bar}"),
            ]
        )
    }

    #[test]
    fn malformed() {
        assert_eq!(
            Tokenizer::new("A ${ab").collect::<Vec<_>>(),
            vec![
                Token::Normal("A "),
                Token::Key("ab", false)
            ]
        );

        assert_eq!(
            Tokenizer::new("${ab}} ${ab${ab").collect::<Vec<_>>(),
            vec![
                Token::Key("ab", false),
                Token::Normal("} "),
                Token::Key("ab${ab", true),
            ]
        )
    }

    #[test]
    fn nested_expander() {
        let pattern = "A ${B \\\\${C}\\\\${D}}";

        fn variable_map(pattern: &str) -> &str {
            match pattern {
                "B \\foo\\bar" => "success",
                "C" => "foo",
                "D" => "bar",
                _ => pattern,
            }
        }

        fn recursive_tokenizer(pattern: &str) -> Result<String, String> {
            Tokenizer::new(pattern).expand(|buf, key| {
                match key {
                    Token::Normal(text) => buf.push_str(text),
                    Token::Escaped(character) => buf.push(character),
                    Token::Key(pattern, tokens_found) => {
                        if tokens_found {
                            let recursed = recursive_tokenizer(pattern)?;
                            buf.push_str(variable_map(&recursed));
                        } else {
                            buf.push_str(variable_map(pattern));
                        }
                    }
                }

                Ok(true)
            })
        }

        assert_eq!(
            recursive_tokenizer(pattern),
            Ok("A success".into())
        );
    }
}

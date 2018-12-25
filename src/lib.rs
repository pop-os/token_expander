#[macro_use]
extern crate derive_new;
#[macro_use]
extern crate smart_default;

pub mod lexer;

use lexer::{Lexer, LexerRules};

const ESCAPED: u8 = 1;

/// Simple, efficient shell-like string tokenizer, and expander extraordinaire.
#[derive(Debug, Clone)]
pub struct Tokenizer<'a> {
    data: &'a str,
    read: usize,
    flags: u8,
    escape: u8,
}

/// An individual token, which may be a variable key, an escaped character, or plain text.
#[derive(Debug, PartialEq)]
pub enum Token<'a> {
    /// The character that follows the escape byte.
    Escaped(char),
    /// The discovered key.
    Key(&'a str),
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
        Tokenizer {
            data,
            read: 0,
            flags: 0,
            escape: b'\\',
        }
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

    fn check_return<S: FnMut(&mut Self), F: FnMut(&mut Self) -> Token<'a>>(
        &mut self,
        start: usize,
        mut do_this: S,
        mut else_apply: F,
    ) -> Token<'a> {
        let token = &self.data[start..self.read];

        if token.is_empty() {
            else_apply(self)
        } else {
            do_this(self);
            Token::Normal(token)
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
    /// let url = "https://$domain/$repo/$name/${name}_${version}_$arch.deb";
    /// assert_eq!(
    ///     Tokenizer::new(url).expand(|buf, key| {
    ///         match key {
    ///             Token::Normal(text)       => buf.push_str(text),
    ///             Token::Key("name")        => buf.push_str("system76"),
    ///             Token::Key("version")     => buf.push_str("1.0.0"),
    ///             Token::Key("arch")        => buf.push_str("amd64"),
    ///             Token::Key("domain")      => buf.push_str("apt.pop-os.org"),
    ///             Token::Key("repo")        => buf.push_str("free"),
    ///             Token::Key(other)         => return Err(format!("unsupported key: {}", other)),
    ///             Token::Escaped('n')       => buf.push('\n'),
    ///             Token::Escaped('t')       => buf.push('\t'),
    ///             Token::Escaped(character) => buf.push(character),
    ///         }
    ///         Ok(true)
    ///     }),
    ///     Ok("https://apt.pop-os.org/free/system76/system76_1.0.0_amd64.deb".into())
    /// );
    /// ```
    fn expand<T, F>(&mut self, mut map: F) -> Result<String, T>
    where
        F: FnMut(&mut String, Token) -> Result<bool, T>,
    {
        let mut output = String::with_capacity(self.len() * 2);
        for token in self {
            if !map(&mut output, token)? {
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

        let start = self.read;
        let bytes = self.data.as_bytes();
        while self.read < self.data.len() {
            match bytes[self.read] {
                byte if byte == self.escape => {
                    return Some(self.check_return(
                        start,
                        |tokenizer| {
                            tokenizer.read += 1;
                            tokenizer.flags |= ESCAPED;
                        },
                        |tokenizer| {
                            tokenizer.read += 1;
                            tokenizer.escaped_character()
                        },
                    ));
                }
                b'$' if bytes.get(self.read + 1) == Some(&b'{') => {
                    return Some(self.check_return(
                        start,
                        |_| {},
                        |tokenizer| {
                            tokenizer.read += 2;
                            let rules = LexerRules::new(b"}", tokenizer.escape);
                            let lexed =
                                Lexer::new(&tokenizer.data[tokenizer.read..], rules).search();
                            tokenizer.read += lexed.len() + 1;
                            Token::Key(lexed)
                        },
                    ));
                }
                b'$' => {
                    return Some(self.check_return(
                        start,
                        |_| {},
                        |tokenizer| {
                            tokenizer.read += 1;
                            const PATTERN: &[u8] = br#"~!@#$%^&*()+-=[]\{}|;':",./<>?"#;
                            let rules = LexerRules::new(PATTERN, tokenizer.escape);
                            let lexed =
                                Lexer::new(&tokenizer.data[tokenizer.read..], rules).search();
                            tokenizer.read += lexed.len();
                            Token::Key(lexed)
                        },
                    ));
                }
                _ => self.read += 1,
            }
        }

        self.read = self.data.len();
        let remaining = &self.data[start..];
        if remaining.is_empty() {
            None
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
        let url = "https://${domain}/$repo/$name/${name}_${version}_$arch.deb";
        assert_eq!(
            Tokenizer::new(url).collect::<Vec<_>>(),
            vec![
                Token::Normal("https://"),
                Token::Key("domain"),
                Token::Normal("/"),
                Token::Key("repo"),
                Token::Normal("/"),
                Token::Key("name"),
                Token::Normal("/"),
                Token::Key("name"),
                Token::Normal("_"),
                Token::Key("version"),
                Token::Normal("_"),
                Token::Key("arch"),
                Token::Normal(".deb"),
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
                    Token::Key("name") => buf.push_str("system76"),
                    Token::Key("version") => buf.push_str("1.0.0"),
                    Token::Key(other) => return Err(format!("unsupported key: {}", other)),
                    Token::Escaped(_) => panic!("didn't expect an escaped character"),
                }

                Ok(true)
            }),
            Ok("https://app.domain.org/system76/system76_1.0.0.deb".into())
        );

        assert_eq!(
            Tokenizer::new("https://app.domain.org/package_version.deb").expand(
                |buf, key| -> Result<bool, String> {
                    match key {
                        Token::Normal(text) => buf.push_str(text),
                        Token::Key(key) if key == "foo" => {
                            buf.push_str("bar");
                        }
                        _ => (),
                    }

                    Ok(true)
                }
            ),
            Ok("https://app.domain.org/package_version.deb".into())
        );
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
            Tokenizer::new("foo###${bar}")
                .set_escape(b'#')
                .collect::<Vec<_>>(),
            vec![
                Token::Normal("foo"),
                Token::Escaped('#'),
                Token::Escaped('$'),
                Token::Normal("{bar}"),
            ]
        );
    }

    #[test]
    fn malformed() {
        assert_eq!(
            Tokenizer::new("A ${ab").collect::<Vec<_>>(),
            vec![Token::Normal("A "), Token::Key("ab")]
        );
    }
}

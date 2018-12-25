#[derive(new, Debug, SmartDefault)]
pub struct LexerRules<'a> {
    stop_on: &'a [u8],
    #[default = b'\\']
    escape: u8,
}

impl<'a> LexerRules<'a> {
    pub fn with_escape(mut self, escape: u8) -> Self {
        self.escape = escape;
        self
    }
}

#[derive(new, Debug, Default)]
pub struct Lexer<'a> {
    search_space: &'a str,
    rules: LexerRules<'a>,
    #[new(default)]
    read: usize,
}

impl<'a> Lexer<'a> {
    pub fn search(&mut self) -> &'a str {
        let start = self.read;
        let mut end = start;

        {
            let mut search = self.search_space.as_bytes()[start..].iter();

            while let Some(byte) = search.next() {
                if *byte == self.rules.escape {
                    end += 2;
                    let _ = search.next();
                    continue;
                } else if self.rules.stop_on.contains(byte) {
                    break;
                } else {
                    end += 1;
                }
            }
        }

        &self.search_space[start..end]
    }
}

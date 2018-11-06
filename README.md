# token-expander

Tokenizer with expansion capabilities for efficiently expanding strings with shell-like variable
expansions. The caller decides how variables and escaped characters are expanded, as well as
which character is to be used as the escape character. Variables are between `${` and `}`.
Quoting rules are not observed in this implementation.

### Tokenizer

The tokenizer may be used by itself, if expansion is to be handled uniquely.

```rust
use token_expander::{Token, Tokenizer};

let url = "https://${domain}/${repo}/${name}/${name}_${version}_${arch}.deb";
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
```

### Expander

Or the token expander functionality can be used to automate the handling of token expansions,
which can be accessed through the `TokenizerExt` trait.

```rust
use token_expander::{Token, Tokenizer, TokenizerExt};

let url = "https://${domain}/${repo}/${name}/${name}_${version}_${arch}.deb";
assert_eq!(
    Tokenizer::new(url).expand(|buf, key| {
        match key {
            Token::Normal(text)       => buf.push_str(text),
            Token::Key("name", _)     => buf.push_str("system76"),
            Token::Key("version", _)  => buf.push_str("1.0.0"),
            Token::Key("arch", _)     => buf.push_str("amd64"),
            Token::Key("domain", _)   => buf.push_str("apt.pop-os.org"),
            Token::Key("repo", _)     => buf.push_str("free"),
            Token::Key(other, _)      => return Err(format!("unsupported key: {}", other)),
            Token::Escaped('n')       => buf.push('\n'),
            Token::Escaped('t')       => buf.push('\t'),
            Token::Escaped(character) => buf.push(character),
        }
        Ok(true)
    }),
    Ok("https://apt.pop-os.org/free/system76/system76_1.0.0_amd64.deb".into())
);
```

### Custom Escape Bytes

Sometimes you may want to alternate between escape characters, depending on the kind of source
input that you need to process.

```rust
use token_expander::{Token, Tokenizer};

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
```

### Nested Expansions

Nested expansion are also a possibility.

```rust
use token_expander::{Token, Tokenizer, TokenizerExt};

let pattern = "A ${B ${C}${D}}";

fn variable_map(pattern: &str) -> &str {
    match pattern {
        "B foobar" => "success",
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
```

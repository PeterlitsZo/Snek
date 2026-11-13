use std::process::Stdio;

use boa_engine::{
    Context, JsArgs, JsNativeError, JsResult, JsValue, js_string,
    object::builtins::{JsArray, JsPromise},
};

fn shell_error(message: impl Into<String>) -> boa_engine::JsError {
    JsNativeError::error().with_message(message.into()).into()
}

/// Convert the template synchronously.
pub(in crate::runtime) fn dollar(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let promise = match template_shell(args, context) {
        Ok(words) => JsPromise::from_async_fn(async move |_| run_shell(words).await, context),
        Err(error) => JsPromise::reject(error, context)?,
    };
    Ok(promise.into())
}

async fn run_shell(words: Vec<String>) -> JsResult<JsValue> {
    let program = &words[0];
    let mut child = tokio::process::Command::new(program)
        .args(&words[1..])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| shell_error(format!("could not start '{program}': {error}")))?;
    let status = child
        .wait()
        .await
        .map_err(|error| shell_error(format!("could not wait for '{program}': {error}")))?;
    if !status.success() {
        return Err(shell_error(format!("'{program}' failed: {status}")));
    }
    Ok(JsValue::undefined())
}

fn template_shell(args: &[JsValue], context: &mut Context) -> JsResult<Vec<String>> {
    let template = args
        .get_or_undefined(0)
        .as_object()
        .ok_or_else(|| shell_error("$ must be called as a tagged template"))?;
    let raw = template
        .get(js_string!("raw"), context)?
        .as_object()
        .ok_or_else(|| shell_error("$ must be called as a tagged template"))?;
    let raw = JsArray::from_object(raw)?;
    if raw.length(context)? != args.len() as u64 || args.is_empty() {
        return Err(shell_error("invalid shell template"));
    }
    // For $`echo pre${value}post`, call segment("echo pre"), interpolate(value),
    // then segment("post"). finish() collects the command and its arguments.
    let mut lexer = ShellLexer::default();
    for (index, argument) in args.iter().enumerate() {
        if index != 0 {
            let value = argument
                .to_string(context)?
                .to_std_string()
                .map_err(|_| shell_error("shell argument contains invalid Unicode"))?;
            lexer.interpolate(&value);
        }
        let text = raw.get(index as u32, context)?;
        let text = text
            .as_string()
            .ok_or_else(|| shell_error("invalid shell template segment"))?
            .to_std_string()
            .map_err(|_| shell_error("shell contains invalid Unicode"))?;
        lexer.segment(&text).map_err(shell_error)?;
    }
    lexer.finish().map_err(shell_error)
}

/// Splits a template into a command and arguments; interpolated values stay literal.
#[derive(Default)]
struct ShellLexer {
    // Completed words; the first is the command name.
    words: Vec<String>,
    // Current word, assembled from template segments and interpolated values.
    word: String,
    // Distinguishes no word from an empty word created by quotes or
    // interpolation.
    started: bool,
    // Open single or double quote, or None when outside quotes.
    quote: Option<char>,
}

impl ShellLexer {
    /// Appends an interpolated value without interpreting spaces, quotes, or
    /// operators. For example, `pre${"a b"}post` produces one argument:
    /// "prea bpost".
    fn interpolate(&mut self, value: &str) {
        self.started = true;
        self.word.push_str(value);
    }

    /// Parses template text: unquoted whitespace separates words; quotes and
    /// escapes protect text. Keeps the current word and quote state across
    /// segments, so quotes can span interpolations.
    fn segment(&mut self, text: &str) -> Result<(), String> {
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            // Inside single quotes, only the closing quote has special meaning.
            if self.quote == Some('\'') {
                if ch == '\'' {
                    self.quote = None;
                } else {
                    self.word.push(ch);
                }
                continue;
            }
            match ch {
                '\\' => {
                    let next = chars.next().ok_or("dangling escape in shell template")?;
                    // In double quotes preserve backslashes except before
                    // quotes, backslashes, dollars, backticks, and line
                    // continuations.
                    if self.quote == Some('"') && !matches!(next, '"' | '\\' | '$' | '`' | '\n') {
                        self.word.push('\\');
                    }
                    if next != '\n' {
                        self.started = true;
                        self.word.push(next);
                    }
                }
                '"' => {
                    self.started = true;
                    self.quote = if self.quote.is_some() { None } else { Some(ch) };
                }
                '\'' if self.quote.is_none() => {
                    self.started = true;
                    self.quote = Some(ch);
                }
                '`' => return Err("shell substitution is not supported".into()),
                '$' if chars.peek() == Some(&'(') => {
                    return Err("shell substitution is not supported".into());
                }
                '|' | '&' | ';' | '<' | '>' if self.quote.is_none() => {
                    return Err(format!(
                        "shell operator '{ch}' is not supported; $ runs a single program"
                    ));
                }
                ch if ch.is_whitespace() && self.quote.is_none() => self.push_word(),
                _ => {
                    self.started = true;
                    self.word.push(ch);
                }
            }
        }
        Ok(())
    }

    /// Collects the current word at a separator or end of input, ignoring
    /// repeated whitespace.
    fn push_word(&mut self) {
        if self.started {
            self.words.push(std::mem::take(&mut self.word));
            self.started = false;
        }
    }

    /// Collects the last word and validates closed quotes, a nonempty command,
    /// and no NUL bytes.
    fn finish(mut self) -> Result<Vec<String>, String> {
        if self.quote.is_some() {
            return Err("unclosed quote in shell template".into());
        }
        self.push_word();
        if self.words.first().is_none_or(String::is_empty) {
            return Err("shell must name a nonempty program".into());
        }
        if self.words.iter().any(|word| word.contains('\0')) {
            return Err("shell arguments cannot contain NUL".into());
        }
        Ok(self.words)
    }
}

#[cfg(test)]
mod tests {
    use super::ShellLexer;

    fn lex(text: &str) -> Result<Vec<String>, String> {
        let mut lexer = ShellLexer::default();
        lexer.segment(text)?;
        lexer.finish()
    }

    #[test]
    fn parses_arguments() {
        let cases: &[(&str, &[&str])] = &[
            (
                " \t echo  hello\n\u{4e16}\u{754c} \r\n",
                &["echo", "hello", "\u{4e16}\u{754c}"],
            ),
            (
                r#"echo 'hello world' "" ''"#,
                &["echo", "hello world", "", ""],
            ),
            (r#"echo pre"mid dle"'post'"#, &["echo", "premid dlepost"]),
            (
                r#"echo '\path "quoted" $(cmd) `cmd` | & ; < >'"#,
                &["echo", r#"\path "quoted" $(cmd) `cmd` | & ; < >"#],
            ),
            (
                r#"echo hello\ world \"\'\\\|\&\;\<\>\`\$(cmd)"#,
                &["echo", "hello world", "\"'\\|&;<>`$(cmd)"],
            ),
            (
                r#"echo "\q \" \\ \$ \` ' | & ; < >""#,
                &["echo", r#"\q " \ $ ` ' | & ; < >"#],
            ),
            (
                "echo \\\n hello\\\nworld \"a\\\nb\"",
                &["echo", "helloworld", "ab"],
            ),
        ];
        for &(input, expected) in cases {
            assert_eq!(lex(input).unwrap(), expected, "{input:?}");
        }
    }

    #[test]
    fn rejects_invalid_syntax() {
        for input in [
            "",
            " \t\n",
            "''",
            "\"\" argument", // Empty program.
            "echo 'unfinished",
            "echo \"unfinished",
            "echo \\",
            "echo \"\\",
            "ec\0ho",
            "echo a\0b",
            "echo $(cmd)",
            "echo `cmd`",
            "echo \"$(cmd)\"",
            "echo \"`cmd`\"",
        ] {
            assert!(lex(input).is_err(), "{input:?}");
        }
        for operator in ['|', '&', ';', '<', '>'] {
            assert_eq!(
                lex(&format!("echo left{operator}right")).unwrap_err(),
                format!("shell operator '{operator}' is not supported; $ runs a single program")
            );
        }
    }

    #[test]
    fn interpolation_stays_literal() {
        for quote in ["", "'", "\""] {
            for value in ["", "hello world\n'\"\\; | & < > $(cmd) `cmd`"] {
                let mut lexer = ShellLexer::default();
                lexer.segment(&format!("echo {quote}")).unwrap();
                lexer.interpolate(value);
                lexer.segment(&format!("{quote} tail")).unwrap();
                assert_eq!(
                    lexer.finish().unwrap(),
                    ["echo", value, "tail"],
                    "{quote:?}, {value:?}"
                );
            }
        }
        let mut lexer = ShellLexer::default();
        lexer.segment("echo pre").unwrap();
        lexer.interpolate("mid dle");
        lexer.segment("post").unwrap();
        assert_eq!(lexer.finish().unwrap(), ["echo", "premid dlepost"]);

        for (prefix, value) in [("", ""), ("echo ", "a\0b")] {
            let mut lexer = ShellLexer::default();
            lexer.segment(prefix).unwrap();
            lexer.interpolate(value);
            assert!(lexer.finish().is_err(), "{prefix:?}, {value:?}");
        }
    }
}

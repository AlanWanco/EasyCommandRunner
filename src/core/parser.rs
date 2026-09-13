/// 命令解析器：把用户粘贴的命令拆成程序、功能和参数行。
///
/// 分词只负责识别 shell 的引号/转义语法，不执行命令；重新生成命令时由
/// `command_line.rs` 按目标平台重新引用参数。
pub struct CommandParser;

impl CommandParser {
    /// 检查字符串是否为标志 (-flag 或 Windows 下的 /flag)。
    fn is_flag(s: &str) -> bool {
        if s.starts_with('-') {
            // -1、-0.5 等负数是参数值，不应该被拆成新的功能行。
            return !s.chars().nth(1).is_some_and(|c| c.is_ascii_digit());
        }
        #[cfg(windows)]
        {
            return s.starts_with('/')
                && s.len() > 1
                && !s[1..].chars().any(|c| matches!(c, '/' | '\\' | ':' | '.'));
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    /// 公开给命令构建器使用的 shell-like 分词器。
    pub fn tokenize(input: &str) -> Result<Vec<String>, String> {
        #[cfg(windows)]
        {
            Self::tokenize_windows(input)
        }
        #[cfg(not(windows))]
        {
            Self::tokenize_posix(input)
        }
    }

    #[cfg(not(windows))]
    fn tokenize_posix(input: &str) -> Result<Vec<String>, String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut quote = None;
        let mut token_started = false;
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            match quote {
                Some('\'') => {
                    if ch == '\'' {
                        quote = None;
                    } else {
                        current.push(ch);
                    }
                }
                Some('"') => match ch {
                    '"' => quote = None,
                    '\\' => {
                        let next = chars.next().ok_or_else(|| "反斜杠后缺少字符".to_string())?;
                        match next {
                            '"' | '\\' | '$' | '`' => current.push(next),
                            '\n' => {}
                            other => {
                                current.push('\\');
                                current.push(other);
                            }
                        }
                    }
                    other => current.push(other),
                },
                Some(_) => current.push(ch),
                None => match ch {
                    '\'' | '"' => {
                        quote = Some(ch);
                        token_started = true;
                    }
                    '\\' => {
                        let next = chars.next().ok_or_else(|| "反斜杠后缺少字符".to_string())?;
                        if next != '\n' {
                            current.push(next);
                        }
                        token_started = true;
                    }
                    whitespace if whitespace.is_whitespace() => {
                        if token_started {
                            parts.push(std::mem::take(&mut current));
                            token_started = false;
                        }
                    }
                    other => {
                        current.push(other);
                        token_started = true;
                    }
                },
            }
        }

        if quote.is_some() {
            return Err("引号没有闭合".to_string());
        }
        if token_started {
            parts.push(current);
        }
        Ok(parts)
    }

    #[cfg(windows)]
    fn tokenize_windows(input: &str) -> Result<Vec<String>, String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut quote = None;
        let mut token_started = false;
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '^' && quote != Some('\'') {
                let next = chars.next().ok_or_else(|| "脱字符后缺少字符".to_string())?;
                current.push(next);
                token_started = true;
                continue;
            }

            if ch == '\\' && quote != Some('\'') {
                let mut count = 1;
                while chars.peek() == Some(&'\\') {
                    chars.next();
                    count += 1;
                }
                if chars.peek() == Some(&'"') {
                    chars.next();
                    current.push_str(&"\\".repeat(count / 2));
                    if count % 2 == 1 {
                        current.push('"');
                    } else if quote == Some('"') {
                        quote = None;
                    } else {
                        quote = Some('"');
                    }
                    token_started = true;
                } else {
                    current.push_str(&"\\".repeat(count));
                    token_started = true;
                }
                continue;
            }

            match quote {
                Some('\'') => {
                    if ch == '\'' {
                        quote = None;
                    } else {
                        current.push(ch);
                    }
                }
                Some('"') => {
                    if ch == '"' {
                        quote = None;
                    } else {
                        current.push(ch);
                    }
                }
                Some(_) => current.push(ch),
                None => match ch {
                    '\'' | '"' => {
                        quote = Some(ch);
                        token_started = true;
                    }
                    whitespace if whitespace.is_whitespace() => {
                        if token_started {
                            parts.push(std::mem::take(&mut current));
                            token_started = false;
                        }
                    }
                    other => {
                        current.push(other);
                        token_started = true;
                    }
                },
            }
        }

        if quote.is_some() {
            return Err("引号没有闭合".to_string());
        }
        if token_started {
            parts.push(current);
        }
        Ok(parts)
    }

    /// 解析完整命令或只包含后续参数的命令。
    pub fn parse(input: &str, is_append: bool) -> Vec<String> {
        let parts = match Self::tokenize(input) {
            Ok(parts) => parts,
            Err(_) => return Vec::new(),
        };
        Self::align(parts, is_append)
    }

    fn align(parts: Vec<String>, is_append: bool) -> Vec<String> {
        let mut new_array = Vec::new();
        for i in 0..parts.len() {
            new_array.push(parts[i].clone());
            if i + 1 < parts.len() && Self::is_flag(&parts[i]) && Self::is_flag(&parts[i + 1]) {
                new_array.push(String::new());
            }
        }

        let mut index = 0;
        for i in 1..new_array.len() {
            if Self::is_flag(&new_array[i]) {
                index = i + 1;
                break;
            }
        }

        if is_append {
            if index > 0 && index % 2 == 0 {
                new_array.insert(index - 1, String::new());
            }
        } else if index > 0 && index % 2 == 1 {
            new_array.insert(index - 1, String::new());
        }

        let mut j = if is_append { 0 } else { 1 };
        while j < new_array.len() {
            if Self::is_flag(&new_array[j]) {
                let mut counter = 0;
                j += 1;
                while j < new_array.len() && !Self::is_flag(&new_array[j]) {
                    counter += 1;
                    j += 1;
                }
                if counter % 2 == 0 {
                    new_array.insert(j, String::new());
                }
            } else {
                j += 1;
            }
        }
        new_array
    }

    /// 保留旧 Rust API，实际构建逻辑集中到跨平台 command_line 模块。
    pub fn build(program: &str, functions: &[(String, String)], other_args: &str) -> String {
        let mut arguments = Vec::new();
        for (func, param) in functions {
            if !func.trim().is_empty() {
                arguments.push(func.clone());
            }
            if !param.trim().is_empty() {
                arguments.push(param.clone());
            }
        }
        crate::core::command_line::CommandLine::build(program, &arguments, other_args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_cases_and_quotes() {
        assert_eq!(
            CommandParser::parse("aaa --arg0 v0 -b c", false),
            ["aaa", "--arg0", "v0", "-b", "c"]
        );
        assert_eq!(
            CommandParser::parse("aaa -x -y", false),
            ["aaa", "-x", "", "-y", ""]
        );
        assert_eq!(CommandParser::parse("-x -y", true), ["-x", "", "-y", ""]);
        assert_eq!(
            CommandParser::parse("tool -a 'two words' -n -1", false),
            ["tool", "-a", "two words", "-n", "-1"]
        );
        assert!(CommandParser::parse("tool -a 'unfinished", false).is_empty());
        assert_eq!(
            CommandParser::parse("tool\t-x\nvalue", false),
            ["tool", "-x", "value"]
        );
    }

    #[test]
    fn test_escaped_spaces_and_quotes() {
        #[cfg(not(windows))]
        assert_eq!(
            CommandParser::tokenize(r#"tool a\ b "quoted value" 'literal $HOME'"#).unwrap(),
            ["tool", "a b", "quoted value", "literal $HOME"]
        );
        #[cfg(windows)]
        assert_eq!(
            CommandParser::tokenize(r#"tool "C:\Program Files\tool.exe" 'quoted value'"#).unwrap(),
            ["tool", r#"C:\Program Files\tool.exe"#, "quoted value"]
        );
    }

    #[test]
    fn test_parse_simple_command() {
        let result = CommandParser::parse("ping www.baidu.com", false);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_build_command() {
        let funcs = vec![("www.baidu.com".to_string(), String::new())];
        let cmd = CommandParser::build("ping", &funcs, "");
        assert!(cmd.contains("ping"));
        assert!(cmd.contains("www.baidu.com"));
    }

    #[test]
    fn test_build_does_not_drop_parameter_without_function() {
        let funcs = vec![(String::new(), "input file.txt".to_string())];
        let cmd = CommandParser::build("cat", &funcs, "");
        assert!(cmd.contains("input file.txt"));
    }
}

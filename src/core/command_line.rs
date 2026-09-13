//! 跨平台命令行的分词和 shell 引用。
//!
//! UI 只负责提供结构化的程序/参数字段，平台相关的空格、引号和反斜杠规则集中在这里。
//! `other_args` 则保留为 shell 片段，以支持管道、重定向和条件执行。

use super::parser::CommandParser;
use std::path::Path;

pub struct CommandLine;

impl CommandLine {
    /// 将结构化参数拼成当前平台 shell 可以重新解析的命令。
    pub fn build(program: &str, arguments: &[String], other_args: &str) -> String {
        let mut parts = Vec::with_capacity(arguments.len() + 2);
        let has_structured_arguments = arguments.iter().any(|argument| !argument.trim().is_empty())
            || !other_args.trim().is_empty();
        if !program.trim().is_empty() {
            let program_text = program.trim();
            if !has_structured_arguments && Self::contains_shell_syntax(program_text) {
                // 兼容用户直接把带管道/变量/重定向的完整命令粘到“程序”框。
                return program_text.to_string();
            }
            let pasted_command = if !has_structured_arguments
                && !Self::contains_shell_syntax(program_text)
                && !Path::new(program_text).exists()
            {
                CommandParser::tokenize(program_text)
                    .ok()
                    .filter(|tokens| tokens.len() > 1)
            } else {
                None
            };
            if let Some(tokens) = pasted_command {
                parts.extend(tokens.into_iter().map(|token| Self::quote_argument(&token)));
            } else {
                parts.push(Self::quote_argument(program_text));
            }
        }

        for argument in arguments {
            if !argument.trim().is_empty() {
                parts.push(Self::quote_argument(argument));
            }
        }

        let other = other_args.trim();
        if !other.is_empty() {
            if Self::contains_shell_syntax(other) {
                // 这是用户明确输入的 shell 片段：不要把 |、>、&& 等当成普通参数引用。
                parts.push(other.to_string());
            } else if let Ok(tokens) = CommandParser::tokenize(other) {
                // 没有 shell 操作符时，其他参数也按多个普通参数处理，修复路径/参数空格问题。
                parts.extend(tokens.into_iter().map(|token| Self::quote_argument(&token)));
            } else {
                // 不完整引号不应该让预览崩溃；原文保留并交给 shell 报错。
                parts.push(other.to_string());
            }
        }

        parts.join(" ")
    }

    /// 引用一个“单独的参数”。输入中的一层外部引号会被视为语法而不是参数内容。
    pub fn quote_argument(value: &str) -> String {
        let value = Self::normalize_argument(value);
        #[cfg(windows)]
        {
            Self::quote_cmd(&value)
        }
        #[cfg(not(windows))]
        {
            Self::quote_posix(&value)
        }
    }

    fn normalize_argument(value: &str) -> String {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return String::new();
        }

        // 兼容旧配置里保存的 "带空格的参数" / '带空格的参数'。
        if let Ok(tokens) = CommandParser::tokenize(trimmed) {
            if tokens.len() == 1
                && (trimmed.starts_with('"')
                    || trimmed.starts_with('\'')
                    || trimmed.contains("\\ "))
            {
                return tokens[0].clone();
            }
        }
        trimmed.to_string()
    }

    fn contains_shell_syntax(value: &str) -> bool {
        value.chars().any(|ch| {
            matches!(
                ch,
                '$' | '%' | '|' | '&' | ';' | '<' | '>' | '`' | '\n' | '\r'
            )
        }) || value.contains("$(")
    }

    #[cfg(not(windows))]
    fn quote_posix(value: &str) -> String {
        if value.is_empty() {
            return "''".to_string();
        }
        if value.bytes().all(Self::is_posix_safe) {
            return value.to_string();
        }

        // 单引号内几乎所有字符都是字面量；单引号本身用 shell 的标准拼接写法表示。
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }

    #[cfg(not(windows))]
    fn is_posix_safe(byte: u8) -> bool {
        byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'_' | b'@' | b'%' | b'+' | b'=' | b':' | b',' | b'.' | b'/' | b'-'
            )
    }

    #[cfg(windows)]
    fn quote_cmd(value: &str) -> String {
        if value.is_empty() {
            return "\"\"".to_string();
        }
        let needs_quotes = value.chars().any(|ch| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '"' | '&' | '|' | '<' | '>' | '(' | ')' | '^' | '%' | '!'
                )
        });
        if !needs_quotes {
            return value.to_string();
        }

        // 这是 Windows 命令行参数的反斜杠/双引号规则，避免 C:\Program Files\... 被拆开。
        let mut result = String::from("\"");
        let mut backslashes = 0;
        for ch in value.chars() {
            if ch == '\\' {
                backslashes += 1;
                continue;
            }
            if ch == '"' {
                result.push_str(&"\\".repeat(backslashes * 2 + 1));
                result.push('"');
            } else {
                result.push_str(&"\\".repeat(backslashes));
                result.push(ch);
            }
            backslashes = 0;
        }
        result.push_str(&"\\".repeat(backslashes * 2));
        result.push('"');
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_spaces_and_shell_metacharacters() {
        let command = CommandLine::build(
            "tool path/program",
            &[
                "input file.txt".to_string(),
                "a'b".to_string(),
                "100% literal".to_string(),
            ],
            "--verbose",
        );
        assert!(command.contains("tool"));
        assert!(command.contains("input"));
        assert!(command.contains("--verbose"));
        #[cfg(not(windows))]
        assert!(command.contains("'a'\"'\"'b'"));
        #[cfg(windows)]
        assert!(command.contains("\"input file.txt\""));
    }

    #[test]
    fn keeps_explicit_shell_fragment_raw() {
        let command = CommandLine::build("echo", &[], "input.txt | grep txt > output.txt");
        assert!(command.ends_with("input.txt | grep txt > output.txt"));
    }

    #[test]
    fn normalizes_old_outer_quotes() {
        #[cfg(not(windows))]
        assert_eq!(CommandLine::quote_argument("\"a b\""), "'a b'");
        #[cfg(windows)]
        assert_eq!(CommandLine::quote_argument("'a b'"), "\"a b\"");
    }

    #[test]
    fn recognizes_pasted_full_command_in_program_field() {
        let command = CommandLine::build("\"tool path\" input.txt", &[], "");
        assert!(command.contains("tool path"));
        assert!(command.ends_with("input.txt"));
    }
}

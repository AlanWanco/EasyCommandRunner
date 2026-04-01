/// 命令解析器 - 从 Python 版本移植
pub struct CommandParser;

impl CommandParser {
    /// 检查字符串是否为标志 (-flag 或 /flag)
    fn is_flag(s: &str) -> bool {
        s.starts_with('-') || s.starts_with('/')
    }

    /// 解析命令字符串为参数列表
    /// 这是从 Python 版本的 analysis() 函数直译过来的
    pub fn parse(input: &str, is_append: bool) -> Vec<String> {
        // 简单分割逻辑：根据空格分割，但保留引号内的内容
        let mut parts: Vec<String> = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;

        for ch in input.chars() {
            match ch {
                '"' => {
                    in_quotes = !in_quotes;
                    current.push(ch);
                }
                ' ' if !in_quotes => {
                    if !current.is_empty() {
                        parts.push(current.clone());
                        current.clear();
                    }
                }
                _ => current.push(ch),
            }
        }

        if !current.is_empty() {
            parts.push(current);
        }

        // 当两个-开头的元素在一起时，中间增加空元素
        let mut new_array = Vec::new();
        for i in 0..parts.len() {
            new_array.push(parts[i].clone());
            if i + 1 < parts.len() && Self::is_flag(&parts[i]) && Self::is_flag(&parts[i + 1]) {
                new_array.push(String::new());
            }
        }

        // 当第一个元素之后的元素数量是奇数时，最后增加空元素
        let mut index = 0;
        for i in 1..new_array.len() {
            if Self::is_flag(&new_array[i]) {
                index = i + 1;
                break;
            }
        }

        if is_append {
            if index > 0 && index % 2 == 0 {
                if index > 1 {
                    new_array.insert(index - 1, String::new());
                }
            }
        } else if index > 0 && index % 2 == 1 {
            if index > 1 {
                new_array.insert(index - 1, String::new());
            }
        }

        // 处理过程中的标志
        let mut j = 0;
        while j < new_array.len() {
            if Self::is_flag(&new_array[j]) {
                let mut counter = 0;
                j += 1;
                while j < new_array.len() && !Self::is_flag(&new_array[j]) {
                    counter += 1;
                    j += 1;
                }
                if counter % 2 == 0 && j <= new_array.len() {
                    new_array.insert(j, String::new());
                }
            } else {
                j += 1;
            }
        }

        new_array
    }

    /// 构建命令字符串
    pub fn build(program: &str, functions: &[(String, String)], other_args: &str) -> String {
        let mut cmd_parts = vec![program.to_string()];

        for (func, param) in functions {
            if !func.is_empty() {
                cmd_parts.push(func.clone());
                if !param.is_empty() {
                    cmd_parts.push(param.clone());
                }
            }
        }

        if !other_args.is_empty() {
            cmd_parts.push(other_args.to_string());
        }

        // 处理空格和引号
        Self::format_command(&cmd_parts)
    }

    fn format_command(parts: &[String]) -> String {
        parts
            .iter()
            .filter(|p| !p.is_empty())
            .map(|p| {
                if p.contains(' ') && !p.starts_with('"') {
                    format!("\"{}\"", p)
                } else {
                    p.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_parse_with_quotes() {
        let result = CommandParser::parse("program \"arg with spaces\" -flag", false);
        assert!(!result.is_empty());
    }
}

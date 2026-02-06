use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct Defs {
    values: HashMap<String, String>,
    defined: HashMap<String, bool>,
}

impl Defs {
    fn new() -> Self {
        Self {
            values: HashMap::new(),
            defined: HashMap::new(),
        }
    }

    fn set_defined(&mut self, key: &str, value: Option<String>) {
        match value {
            Some(v) => {
                self.values.insert(key.to_string(), v);
                self.defined.insert(key.to_string(), true);
            }
            None => {
                self.values.remove(key);
                self.defined.insert(key.to_string(), false);
            }
        }
    }

    fn is_defined(&self, key: &str) -> bool {
        self.defined.get(key).copied().unwrap_or(false)
    }

    fn get_value(&self, key: &str) -> String {
        if self.is_defined(key) {
            self.values.get(key).cloned().unwrap_or_else(|| "TRUE".to_string())
        } else {
            String::new()
        }
    }
}

#[derive(Debug)]
struct CondFrame {
    parent_active: bool,
    active: bool,
    else_seen: bool,
}

fn main() {
    let mut defs = Defs::new();
    let mut input: Option<String> = None;

    for arg in env::args().skip(1) {
        if let Some(rest) = arg.strip_prefix("-D") {
            if rest.is_empty() {
                continue;
            }
            if let Some((k, v)) = rest.split_once('=') {
                if v.is_empty() {
                    defs.set_defined(k, None);
                } else {
                    defs.set_defined(k, Some(v.to_string()));
                }
            } else {
                defs.set_defined(rest, Some("TRUE".to_string()));
            }
        } else if input.is_none() {
            input = Some(arg);
        }
    }

    let input = match input {
        Some(v) => v,
        None => {
            eprintln!("usage: textpp [-DKEY[=VALUE]] <input-file>");
            std::process::exit(2);
        }
    };

    let input_path = PathBuf::from(&input);
    let mut out = String::new();
    match process_file(&input_path, &defs, &mut out) {
        Ok(()) => {
            print!("{out}");
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

fn process_file(path: &Path, defs: &Defs, out: &mut String) -> Result<(), String> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut stack: Vec<CondFrame> = Vec::new();
    let mut current_active = true;

    for raw_line in content.lines() {
        if let Some(rest) = raw_line.strip_prefix('#') {
            let trimmed = rest.trim_start();
            if trimmed.starts_with("include") {
                if current_active {
                    if let Some(include_path) = parse_include_path(trimmed, defs) {
                        let joined = base_dir.join(include_path);
                        let _ = process_file(&joined, defs, out);
                    }
                }
                continue;
            }
            if trimmed.starts_with("ifdef") {
                let name = trimmed["ifdef".len()..].trim();
                let cond = defs.is_defined(name);
                let new_active = current_active && cond;
                stack.push(CondFrame {
                    parent_active: current_active,
                    active: cond,
                    else_seen: false,
                });
                current_active = new_active;
                continue;
            }
            if trimmed.starts_with("ifndef") {
                let name = trimmed["ifndef".len()..].trim();
                let cond = !defs.is_defined(name);
                let new_active = current_active && cond;
                stack.push(CondFrame {
                    parent_active: current_active,
                    active: cond,
                    else_seen: false,
                });
                current_active = new_active;
                continue;
            }
            if trimmed.starts_with("if") {
                let expr = trimmed["if".len()..].trim();
                let cond = eval_expr(expr, defs)?;
                let new_active = current_active && cond;
                stack.push(CondFrame {
                    parent_active: current_active,
                    active: cond,
                    else_seen: false,
                });
                current_active = new_active;
                continue;
            }
            if trimmed.starts_with("else") {
                let top = stack.last_mut().ok_or_else(|| {
                    "invalid directive structure: #else without matching #if/#ifdef/#ifndef"
                        .to_string()
                })?;
                if !top.else_seen {
                    top.else_seen = true;
                    top.active = !top.active;
                    current_active = top.parent_active && top.active;
                }
                continue;
            }
            if trimmed.starts_with("endif") {
                let top = stack.pop().ok_or_else(|| {
                    "invalid directive structure: #endif without matching #if/#ifdef/#ifndef"
                        .to_string()
                })?;
                current_active = top.parent_active;
                continue;
            }
        }

        if current_active {
            let replaced = replace_dollar_vars(raw_line, defs);
            out.push_str(&replaced);
            out.push('\n');
        }
    }

    if !stack.is_empty() {
        return Err("invalid directive structure: missing #endif".to_string());
    }

    Ok(())
}

fn parse_include_path(rest: &str, defs: &Defs) -> Option<PathBuf> {
    let after = rest["include".len()..].trim();
    if after.is_empty() {
        return None;
    }
    let mut cleaned = after.to_string();
    cleaned.retain(|c| c != '"');
    let replaced = replace_hash_vars(&cleaned, defs);
    if replaced.is_empty() {
        None
    } else {
        Some(PathBuf::from(replaced))
    }
}

fn replace_hash_vars(input: &str, defs: &Defs) -> String {
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let bytes = input.as_bytes();
    while i + 1 < bytes.len() {
        if bytes[i] == b'#' && bytes[i + 1] == b'#' {
            if let Some(end) = find_double_hash_end(bytes, i + 2) {
                let name = &input[i + 2..end];
                if is_ident(name) && defs.is_defined(name) {
                    out.push_str(&defs.get_value(name));
                }
                i = end + 2;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    if i < bytes.len() {
        out.push(bytes[i] as char);
    }
    out
}

fn find_double_hash_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut j = start;
    while j + 1 < bytes.len() {
        if bytes[j] == b'#' && bytes[j + 1] == b'#' {
            return Some(j);
        }
        j += 1;
    }
    None
}

fn replace_dollar_vars(input: &str, defs: &Defs) -> String {
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let bytes = input.as_bytes();
    while i + 1 < bytes.len() {
        if bytes[i] == b'$' && bytes[i + 1] == b'$' {
            if let Some(end) = find_double_dollar_end(bytes, i + 2) {
                let name = &input[i + 2..end];
                if is_ident(name) {
                    out.push_str(&defs.get_value(name));
                }
                i = end + 2;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    if i < bytes.len() {
        out.push(bytes[i] as char);
    }
    out
}

fn find_double_dollar_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut j = start;
    while j + 1 < bytes.len() {
        if bytes[j] == b'$' && bytes[j + 1] == b'$' {
            return Some(j);
        }
        j += 1;
    }
    None
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return false;
        }
    }
    true
}

fn truthy(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    let upper = value.to_ascii_uppercase();
    !(upper == "0" || upper == "F" || upper == "FALSE" || upper == "NO")
}

#[derive(Debug, Clone)]
enum Token {
    Ident(String),
    Str(String),
    Num(String),
    And,
    Or,
    Eq,
    Ne,
    Not,
    LParen,
    RParen,
}

fn eval_expr(expr: &str, defs: &Defs) -> Result<bool, String> {
    let tokens = tokenize(expr)?;
    let mut parser = Parser { tokens: &tokens, pos: 0, defs };
    let value = parser.parse_or()?;
    if parser.pos != tokens.len() {
        return Err(format!("invalid expression: unexpected token at position {}", parser.pos));
    }
    Ok(value)
}

fn tokenize(expr: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = expr.chars().collect();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '&' => {
                if i + 1 < chars.len() && chars[i + 1] == '&' {
                    tokens.push(Token::And);
                    i += 2;
                } else {
                    return Err("invalid expression: single '&'".to_string());
                }
            }
            '|' => {
                if i + 1 < chars.len() && chars[i + 1] == '|' {
                    tokens.push(Token::Or);
                    i += 2;
                } else {
                    return Err("invalid expression: single '|'".to_string());
                }
            }
            '=' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::Eq);
                    i += 2;
                } else {
                    return Err("invalid expression: single '='".to_string());
                }
            }
            '!' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::Ne);
                    i += 2;
                } else {
                    tokens.push(Token::Not);
                    i += 1;
                }
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '"' => {
                i += 1;
                let mut s = String::new();
                while i < chars.len() {
                    let ch = chars[i];
                    if ch == '"' {
                        break;
                    }
                    if ch == '\\' && i + 1 < chars.len() {
                        let next = chars[i + 1];
                        s.push(next);
                        i += 2;
                        continue;
                    }
                    s.push(ch);
                    i += 1;
                }
                if i >= chars.len() || chars[i] != '"' {
                    return Err("invalid expression: unterminated string".to_string());
                }
                i += 1;
                tokens.push(Token::Str(s));
            }
            c if c.is_ascii_digit() => {
                let mut s = String::new();
                s.push(c);
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    s.push(chars[i]);
                    i += 1;
                }
                tokens.push(Token::Num(s));
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut s = String::new();
                s.push(c);
                i += 1;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    s.push(chars[i]);
                    i += 1;
                }
                tokens.push(Token::Ident(s));
            }
            _ => return Err(format!("invalid expression: unexpected char '{c}'")),
        }
    }
    Ok(tokens)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    defs: &'a Defs,
}

impl<'a> Parser<'a> {
    fn parse_or(&mut self) -> Result<bool, String> {
        let mut left = self.parse_and()?;
        while self.match_token(|t| matches!(t, Token::Or)) {
            let right = self.parse_and()?;
            left = left || right;
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<bool, String> {
        let mut left = self.parse_not()?;
        while self.match_token(|t| matches!(t, Token::And)) {
            let right = self.parse_not()?;
            left = left && right;
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<bool, String> {
        if self.match_token(|t| matches!(t, Token::Not)) {
            let v = self.parse_not()?;
            return Ok(!v);
        }
        self.parse_cmp()
    }

    fn parse_cmp(&mut self) -> Result<bool, String> {
        if self.match_token(|t| matches!(t, Token::LParen)) {
            let v = self.parse_or()?;
            if !self.match_token(|t| matches!(t, Token::RParen)) {
                return Err("invalid expression: missing ')'".to_string());
            }
            return Ok(v);
        }
        let left = self.parse_value()?;
        if self.match_token(|t| matches!(t, Token::Eq)) {
            let right = self.parse_value()?;
            return Ok(left == right);
        }
        if self.match_token(|t| matches!(t, Token::Ne)) {
            let right = self.parse_value()?;
            return Ok(left != right);
        }
        Ok(truthy(&left))
    }

    fn parse_value(&mut self) -> Result<String, String> {
        if let Some(token) = self.tokens.get(self.pos) {
            let value = match token {
                Token::Ident(name) => self.defs.get_value(name),
                Token::Str(s) => s.clone(),
                Token::Num(n) => n.clone(),
                _ => return Err("invalid expression: expected value".to_string()),
            };
            self.pos += 1;
            return Ok(value);
        }
        Err("invalid expression: unexpected end".to_string())
    }

    fn match_token<F>(&mut self, pred: F) -> bool
    where
        F: Fn(&Token) -> bool,
    {
        if let Some(tok) = self.tokens.get(self.pos) {
            if pred(tok) {
                self.pos += 1;
                return true;
            }
        }
        false
    }
}

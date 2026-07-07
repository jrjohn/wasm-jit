//! Minimal DSL parser. The DSL is deliberately a strict subset of Rhai syntax
//! so the SAME source text runs on both the Rhai interpreter and our compiler.
//!
//! Grammar:
//!   program := stmt* expr EOF          (final bare expression is the return value)
//!   stmt    := "let" ident "=" expr ";"
//!            | ident "=" expr ";"
//!            | "while" expr "{" stmt* "}"
//!   expr    := add (("<"|">"|"<="|">=") add)?
//!   add     := mul (("+"|"-") mul)*
//!   mul     := unary (("*"|"/") unary)*
//!   unary   := "-" unary | primary
//!   primary := number | ident | "(" expr ")"
//!
//! All values are f64. Comparisons yield 1.0/0.0 in value position.

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Num(f64),
    Ident(String),
    Let,
    While,
    Plus,
    Minus,
    Star,
    Slash,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Semi,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Gt,
    Le,
    Ge,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Num(f64),
    Var(String),
    Binary(Box<Expr>, BinOp, Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let(String, Expr),
    Assign(String, Expr),
    While(Expr, Vec<Stmt>),
}

#[derive(Debug, Clone)]
pub struct Program {
    pub stmts: Vec<Stmt>,
    pub ret: Expr,
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let b = src.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        match c {
            ' ' | '\t' | '\r' | '\n' => i += 1,
            '+' => {
                toks.push(Tok::Plus);
                i += 1;
            }
            '-' => {
                toks.push(Tok::Minus);
                i += 1;
            }
            '*' => {
                toks.push(Tok::Star);
                i += 1;
            }
            '/' => {
                if i + 1 < b.len() && b[i + 1] == b'/' {
                    while i < b.len() && b[i] != b'\n' {
                        i += 1;
                    }
                } else {
                    toks.push(Tok::Slash);
                    i += 1;
                }
            }
            '<' => {
                if i + 1 < b.len() && b[i + 1] == b'=' {
                    toks.push(Tok::Le);
                    i += 2;
                } else {
                    toks.push(Tok::Lt);
                    i += 1;
                }
            }
            '>' => {
                if i + 1 < b.len() && b[i + 1] == b'=' {
                    toks.push(Tok::Ge);
                    i += 2;
                } else {
                    toks.push(Tok::Gt);
                    i += 1;
                }
            }
            '=' => {
                toks.push(Tok::Eq);
                i += 1;
            }
            ';' => {
                toks.push(Tok::Semi);
                i += 1;
            }
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            '{' => {
                toks.push(Tok::LBrace);
                i += 1;
            }
            '}' => {
                toks.push(Tok::RBrace);
                i += 1;
            }
            '0'..='9' | '.' => {
                let start = i;
                while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                    i += 1;
                }
                let s = &src[start..i];
                let v: f64 = s.parse().map_err(|_| format!("bad number literal '{s}'"))?;
                toks.push(Tok::Num(v));
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let start = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                let s = &src[start..i];
                toks.push(match s {
                    "let" => Tok::Let,
                    "while" => Tok::While,
                    _ => Tok::Ident(s.to_string()),
                });
            }
            _ => return Err(format!("unexpected character '{c}' at byte {i}")),
        }
    }
    toks.push(Tok::Eof);
    Ok(toks)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos]
    }
    fn peek2(&self) -> &Tok {
        self.toks.get(self.pos + 1).unwrap_or(&Tok::Eof)
    }
    fn advance(&mut self) -> Tok {
        let t = self.toks[self.pos].clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }
    fn expect(&mut self, t: &Tok, what: &str) -> Result<(), String> {
        if self.peek() == t {
            self.advance();
            Ok(())
        } else {
            Err(format!("expected {what}, found {:?}", self.peek()))
        }
    }

    fn parse_program(&mut self) -> Result<Program, String> {
        let mut stmts = Vec::new();
        loop {
            match self.peek() {
                Tok::Let | Tok::While => stmts.push(self.parse_stmt()?),
                Tok::Ident(_) if matches!(self.peek2(), Tok::Eq) => {
                    stmts.push(self.parse_stmt()?)
                }
                Tok::Eof => return Err("expected a final expression (return value)".into()),
                _ => break,
            }
        }
        let ret = self.parse_expr()?;
        if !matches!(self.peek(), Tok::Eof) {
            return Err(format!(
                "unexpected token after final expression: {:?} (the final expression must be last and have no ';')",
                self.peek()
            ));
        }
        Ok(Program { stmts, ret })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        match self.peek().clone() {
            Tok::Let => {
                self.advance();
                let name = match self.advance() {
                    Tok::Ident(s) => s,
                    t => return Err(format!("expected identifier after 'let', found {t:?}")),
                };
                self.expect(&Tok::Eq, "'='")?;
                let e = self.parse_expr()?;
                self.expect(&Tok::Semi, "';'")?;
                Ok(Stmt::Let(name, e))
            }
            Tok::While => {
                self.advance();
                let cond = self.parse_expr()?;
                self.expect(&Tok::LBrace, "'{'")?;
                let mut body = Vec::new();
                while !matches!(self.peek(), Tok::RBrace) {
                    if matches!(self.peek(), Tok::Eof) {
                        return Err("unterminated while body: missing '}'".into());
                    }
                    body.push(self.parse_stmt()?);
                }
                self.expect(&Tok::RBrace, "'}'")?;
                Ok(Stmt::While(cond, body))
            }
            Tok::Ident(name) => {
                self.advance();
                self.expect(&Tok::Eq, "'='")?;
                let e = self.parse_expr()?;
                self.expect(&Tok::Semi, "';'")?;
                Ok(Stmt::Assign(name, e))
            }
            t => Err(format!("expected statement, found {t:?}")),
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        let lhs = self.parse_add()?;
        let op = match self.peek() {
            Tok::Lt => BinOp::Lt,
            Tok::Gt => BinOp::Gt,
            Tok::Le => BinOp::Le,
            Tok::Ge => BinOp::Ge,
            _ => return Ok(lhs),
        };
        self.advance();
        let rhs = self.parse_add()?;
        Ok(Expr::Binary(Box::new(lhs), op, Box::new(rhs)))
    }

    fn parse_add(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => return Ok(lhs),
            };
            self.advance();
            let rhs = self.parse_mul()?;
            lhs = Expr::Binary(Box::new(lhs), op, Box::new(rhs));
        }
    }

    fn parse_mul(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                _ => return Ok(lhs),
            };
            self.advance();
            let rhs = self.parse_unary()?;
            lhs = Expr::Binary(Box::new(lhs), op, Box::new(rhs));
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if matches!(self.peek(), Tok::Minus) {
            self.advance();
            let e = self.parse_unary()?;
            return Ok(Expr::Binary(
                Box::new(Expr::Num(0.0)),
                BinOp::Sub,
                Box::new(e),
            ));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.advance() {
            Tok::Num(v) => Ok(Expr::Num(v)),
            Tok::Ident(s) => Ok(Expr::Var(s)),
            Tok::LParen => {
                let e = self.parse_expr()?;
                self.expect(&Tok::RParen, "')'")?;
                Ok(e)
            }
            t => Err(format!("expected number, variable or '(', found {t:?}")),
        }
    }
}

pub fn parse(src: &str) -> Result<Program, String> {
    let toks = lex(src)?;
    Parser { toks, pos: 0 }.parse_program()
}

/// Transpile the AST to a JS function body (`new Function('n', body)`).
/// Comparisons rely on JS bool→number coercion, matching the DSL's 1.0/0.0.
pub fn to_js(prog: &Program) -> String {
    fn expr_js(e: &Expr, out: &mut String) {
        match e {
            Expr::Num(v) => out.push_str(&format!("{v:?}")),
            Expr::Var(s) => out.push_str(s),
            Expr::Binary(l, op, r) => {
                out.push('(');
                expr_js(l, out);
                out.push_str(match op {
                    BinOp::Add => "+",
                    BinOp::Sub => "-",
                    BinOp::Mul => "*",
                    BinOp::Div => "/",
                    BinOp::Lt => "<",
                    BinOp::Gt => ">",
                    BinOp::Le => "<=",
                    BinOp::Ge => ">=",
                });
                expr_js(r, out);
                out.push(')');
            }
        }
    }
    fn stmt_js(s: &Stmt, out: &mut String) {
        match s {
            Stmt::Let(name, e) => {
                out.push_str("let ");
                out.push_str(name);
                out.push('=');
                expr_js(e, out);
                out.push_str(";\n");
            }
            Stmt::Assign(name, e) => {
                out.push_str(name);
                out.push('=');
                expr_js(e, out);
                out.push_str(";\n");
            }
            Stmt::While(cond, body) => {
                out.push_str("while(");
                expr_js(cond, out);
                out.push_str("){\n");
                for s in body {
                    stmt_js(s, out);
                }
                out.push_str("}\n");
            }
        }
    }
    let mut out = String::new();
    for s in &prog.stmts {
        stmt_js(s, &mut out);
    }
    out.push_str("return ");
    expr_js(&prog.ret, &mut out);
    out.push(';');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tiny const-evaluator for tests (no variables).
    fn eval_const(e: &Expr) -> f64 {
        match e {
            Expr::Num(v) => *v,
            Expr::Var(_) => panic!("no vars in const test"),
            Expr::Binary(l, op, r) => {
                let (a, b) = (eval_const(l), eval_const(r));
                match op {
                    BinOp::Add => a + b,
                    BinOp::Sub => a - b,
                    BinOp::Mul => a * b,
                    BinOp::Div => a / b,
                    BinOp::Lt => (a < b) as i32 as f64,
                    BinOp::Gt => (a > b) as i32 as f64,
                    BinOp::Le => (a <= b) as i32 as f64,
                    BinOp::Ge => (a >= b) as i32 as f64,
                }
            }
        }
    }

    #[test]
    fn precedence() {
        let p = parse("1.0 + 2.0 * 3.0").unwrap();
        assert_eq!(eval_const(&p.ret), 7.0);
        let p = parse("(1.0 + 2.0) * 3.0").unwrap();
        assert_eq!(eval_const(&p.ret), 9.0);
        let p = parse("-2.0 * 3.0").unwrap();
        assert_eq!(eval_const(&p.ret), -6.0);
        let p = parse("1.0 < 2.0").unwrap();
        assert_eq!(eval_const(&p.ret), 1.0);
    }

    #[test]
    fn default_kernel_parses() {
        let src = "let sum = 0.0;\nlet i = 0.0;\nwhile i < n {\n sum = sum + i * i - sum / (i + 1.0);\n i = i + 1.0;\n}\nsum";
        let p = parse(src).unwrap();
        assert_eq!(p.stmts.len(), 3);
        assert!(matches!(p.stmts[2], Stmt::While(_, ref body) if body.len() == 2));
        assert!(matches!(p.ret, Expr::Var(ref s) if s == "sum"));
    }

    #[test]
    fn js_transpile_shape() {
        let src = "let x = 1.0; while x < n { x = x * 2.0; } x";
        let js = to_js(&parse(src).unwrap());
        assert!(js.contains("while("));
        assert!(js.trim_end().ends_with("return x;"));
    }

    #[test]
    fn errors() {
        assert!(parse("let x = ;").is_err());
        assert!(parse("").is_err());
        assert!(parse("let x = 1.0; x = 2.0;").is_err()); // no final expression
    }
}

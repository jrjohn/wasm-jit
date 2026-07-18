//! Minimal DSL parser for the seed language — a small f64-scalar language
//! that compiles to WASM. Deliberately tiny so its whole contract fits in a
//! prompt and "what this code can touch" is a compile-time-enumerable list.
//!
//! Grammar:
//!   program := stmt* expr EOF          (final bare expression is the return value)
//!   stmt    := "let" ident "=" expr ";"
//!            | ident "=" expr ";"
//!            | "while" expr "{" stmt* "}"
//!            | ident "(" args ")" ";"          (void host-fn call, e.g. out(x,y);)
//!   expr    := add (("<"|">"|"<="|">=") add)?
//!   add     := mul (("+"|"-") mul)*
//!   mul     := unary (("*"|"/") unary)*
//!   unary   := "-" unary | primary
//!   primary := number | ident | ident "(" args ")" | "(" expr ")"
//!
//! All values are f64. Comparisons yield 1.0/0.0 in value position.

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Num(f64),
    Ident(String),
    Let,
    While,
    If,
    Else,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Semi,
    Comma,
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
    Rem,
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
    /// Host-function call, e.g. `sin(x)`. Resolved against the import table at codegen.
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let(String, Expr),
    Assign(String, Expr),
    While(Expr, Vec<Stmt>),
    /// Void host-function call statement, e.g. `out(x, y);`
    Call(String, Vec<Expr>),
    /// `if cond { … } else { … }` (the `else` is optional and may chain into `else if`)
    If(Expr, Vec<Stmt>, Vec<Stmt>),
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
            '%' => {
                toks.push(Tok::Percent);
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
            ',' => {
                toks.push(Tok::Comma);
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
                    "if" => Tok::If,
                    "else" => Tok::Else,
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
                Tok::Let | Tok::While | Tok::If => stmts.push(self.parse_stmt()?),
                Tok::Ident(_) if matches!(self.peek2(), Tok::Eq) => {
                    stmts.push(self.parse_stmt()?)
                }
                Tok::Eof => return Err("expected a final expression (return value)".into()),
                _ => {
                    // Could be a call statement (`out(x, y);`) or the final expression.
                    let e = self.parse_expr()?;
                    if matches!(self.peek(), Tok::Semi) {
                        match e {
                            Expr::Call(name, args) => {
                                self.advance();
                                stmts.push(Stmt::Call(name, args));
                                continue;
                            }
                            _ => return Err(
                                "only call expressions may be used as statements".into(),
                            ),
                        }
                    }
                    if !matches!(self.peek(), Tok::Eof) {
                        return Err(format!(
                            "unexpected token after final expression: {:?} (the final expression must be last and have no ';')",
                            self.peek()
                        ));
                    }
                    return Ok(Program { stmts, ret: e });
                }
            }
        }
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
                let body = self.parse_block()?;
                Ok(Stmt::While(cond, body))
            }
            Tok::If => {
                self.advance();
                let cond = self.parse_expr()?;
                let then_body = self.parse_block()?;
                let else_body = if matches!(self.peek(), Tok::Else) {
                    self.advance();
                    if matches!(self.peek(), Tok::If) {
                        vec![self.parse_stmt()?] // else-if chain
                    } else {
                        self.parse_block()?
                    }
                } else {
                    Vec::new()
                };
                Ok(Stmt::If(cond, then_body, else_body))
            }
            Tok::Ident(name) => {
                if matches!(self.peek2(), Tok::LParen) {
                    // call statement inside a block, e.g. `out(x, y);`
                    let e = self.parse_expr()?;
                    return match e {
                        Expr::Call(name, args) => {
                            self.expect(&Tok::Semi, "';'")?;
                            Ok(Stmt::Call(name, args))
                        }
                        _ => Err("only call expressions may be used as statements".into()),
                    };
                }
                self.advance();
                self.expect(&Tok::Eq, "'='")?;
                let e = self.parse_expr()?;
                self.expect(&Tok::Semi, "';'")?;
                Ok(Stmt::Assign(name, e))
            }
            t => Err(format!("expected statement, found {t:?}")),
        }
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, String> {
        self.expect(&Tok::LBrace, "'{'")?;
        let mut body = Vec::new();
        while !matches!(self.peek(), Tok::RBrace) {
            if matches!(self.peek(), Tok::Eof) {
                return Err("unterminated block: missing '}'".into());
            }
            body.push(self.parse_stmt()?);
        }
        self.expect(&Tok::RBrace, "'}'")?;
        Ok(body)
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
                Tok::Percent => BinOp::Rem,
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
            Tok::Ident(s) => {
                if matches!(self.peek(), Tok::LParen) {
                    self.advance(); // consume '('
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Tok::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if matches!(self.peek(), Tok::Comma) {
                                self.advance();
                                continue;
                            }
                            break;
                        }
                    }
                    self.expect(&Tok::RParen, "')'")?;
                    return Ok(Expr::Call(s, args));
                }
                Ok(Expr::Var(s))
            }
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

/// DSL → GLSL ES 3.00 fragment-shader body (the L4 lane). Same AST as to_js;
/// user identifiers are prefixed `v_` so they can never collide with GLSL
/// keywords/builtins. Value-position comparisons become float(a<b); `%` maps
/// to mod(). Allowed calls: pure math (native), the colour verbs (map to the
/// output colour `_oc`), and the pointer (map to uniforms). Anything else —
/// get/set/disc/fetch/… — is rejected here: a pixel has no memory and no reach.
pub fn to_glsl(prog: &Program) -> Result<String, String> {
    const MATH: [&str; 7] = ["sin", "cos", "min", "max", "abs", "sqrt", "floor"];
    fn expr(e: &Expr, out: &mut String) -> Result<(), String> {
        match e {
            Expr::Num(v) => {
                let mut lit = format!("{v:?}");
                if !lit.contains('.') && !lit.contains('e') { lit.push_str(".0"); }
                out.push_str(&lit);
            }
            Expr::Var(s) => { out.push_str("v_"); out.push_str(s); }
            Expr::Binary(l, op, r) => {
                let cmp = matches!(op, BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge);
                if cmp { out.push_str("float"); }
                out.push('(');
                if matches!(op, BinOp::Rem) {
                    out.push_str("mod(");
                    expr(l, out)?;
                    out.push(',');
                    expr(r, out)?;
                    out.push(')');
                } else {
                    expr(l, out)?;
                    out.push_str(match op {
                        BinOp::Add => "+", BinOp::Sub => "-", BinOp::Mul => "*",
                        BinOp::Div => "/", BinOp::Rem => unreachable!(),
                        BinOp::Lt => "<", BinOp::Gt => ">", BinOp::Le => "<=", BinOp::Ge => ">=",
                    });
                    expr(r, out)?;
                }
                out.push(')');
            }
            Expr::Call(name, args) => {
                match name.as_str() {
                    n if MATH.contains(&n) => {
                        out.push_str(n);
                        out.push('(');
                        for (k, a) in args.iter().enumerate() {
                            if k > 0 { out.push(','); }
                            expr(a, out)?;
                        }
                        out.push(')');
                    }
                    "mx" => out.push_str("uMx"),
                    "my" => out.push_str("uMy"),
                    "down" => out.push_str("uDown"),
                    other => return Err(format!("'{other}' does not exist in the shader fence")),
                }
            }
        }
        Ok(())
    }
    fn stmts(list: &[Stmt], out: &mut String) -> Result<(), String> {
        for s in list {
            match s {
                Stmt::Let(name, e) => {
                    out.push_str("float v_");
                    out.push_str(name);
                    out.push('=');
                    expr(e, out)?;
                    out.push_str(";\n");
                }
                Stmt::Assign(name, e) => {
                    out.push_str("v_");
                    out.push_str(name);
                    out.push('=');
                    expr(e, out)?;
                    out.push_str(";\n");
                }
                Stmt::While(cond, body) => {
                    out.push_str("while((");
                    expr(cond, out)?;
                    out.push_str(")!=0.0){\n");
                    stmts(body, out)?;
                    out.push_str("}\n");
                }
                Stmt::If(cond, tb, eb) => {
                    out.push_str("if((");
                    expr(cond, out)?;
                    out.push_str(")!=0.0){\n");
                    stmts(tb, out)?;
                    out.push_str("}\n");
                    if !eb.is_empty() {
                        out.push_str("else{\n");
                        stmts(eb, out)?;
                        out.push_str("}\n");
                    }
                }
                Stmt::Call(name, args) => match name.as_str() {
                    // the colour verbs: the only way a pixel speaks
                    "rgb" => {
                        if args.len() != 3 { return Err("rgb needs 3 args".into()); }
                        out.push_str("_oc=vec3(");
                        for (k, a) in args.iter().enumerate() {
                            if k > 0 { out.push(','); }
                            out.push_str("clamp(");
                            expr(a, out)?;
                            out.push_str(",0.0,1.0)");
                        }
                        out.push_str(");\n");
                    }
                    "hsl" => {
                        if args.len() != 3 { return Err("hsl needs 3 args".into()); }
                        out.push_str("_oc=_hsl(");
                        for (k, a) in args.iter().enumerate() {
                            if k > 0 { out.push(','); }
                            expr(a, out)?;
                        }
                        out.push_str(");\n");
                    }
                    "hue" => {
                        if args.len() != 1 { return Err("hue needs 1 arg".into()); }
                        out.push_str("_oc=_hsl(");
                        expr(&args[0], out)?;
                        out.push_str(",0.62,0.62);\n");
                    }
                    other => return Err(format!("'{other}' does not exist in the shader fence")),
                },
            }
        }
        Ok(())
    }
    let mut body = String::new();
    stmts(&prog.stmts, &mut body)?;
    // the return value is unused (colour flows through _oc), but it must parse
    let mut ret = String::new();
    expr(&prog.ret, &mut ret)?;
    body.push_str("_unused=");
    body.push_str(&ret);
    body.push_str(";\n");
    Ok(body)
}

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
                    BinOp::Rem => "%",
                    BinOp::Lt => "<",
                    BinOp::Gt => ">",
                    BinOp::Le => "<=",
                    BinOp::Ge => ">=",
                });
                expr_js(r, out);
                out.push(')');
            }
            Expr::Call(name, args) => {
                // Built-in math functions map to Math.* on the JS side (the DSL has no member access, so there's no name collision).
                match name.as_str() {
                    "min" | "max" | "abs" | "sqrt" | "floor" => {
                        out.push_str("Math.");
                        out.push_str(name);
                    }
                    _ => out.push_str(name),
                }
                out.push('(');
                for (k, a) in args.iter().enumerate() {
                    if k > 0 {
                        out.push(',');
                    }
                    expr_js(a, out);
                }
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
            Stmt::Call(name, args) => {
                expr_js(&Expr::Call(name.clone(), args.clone()), out);
                out.push_str(";\n");
            }
            Stmt::If(cond, then_body, else_body) => {
                out.push_str("if(");
                expr_js(cond, out);
                out.push_str("){\n");
                for s in then_body {
                    stmt_js(s, out);
                }
                out.push('}');
                if !else_body.is_empty() {
                    out.push_str("else{\n");
                    for s in else_body {
                        stmt_js(s, out);
                    }
                    out.push('}');
                }
                out.push('\n');
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
            Expr::Call(_, _) => panic!("no calls in const test"),
            Expr::Binary(l, op, r) => {
                let (a, b) = (eval_const(l), eval_const(r));
                match op {
                    BinOp::Add => a + b,
                    BinOp::Sub => a - b,
                    BinOp::Mul => a * b,
                    BinOp::Div => a / b,
                    BinOp::Rem => a - (a / b).trunc() * b,
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
        let p = parse("7.5 % 2.0").unwrap();
        assert_eq!(eval_const(&p.ret), 1.5);
    }

    #[test]
    fn if_else_chain_parses() {
        let src = "let x = 0.0; if n > 2.0 { x = 1.0; } else if n > 1.0 { x = 0.5; } else { x = 0.0; } x";
        let p = parse(src).unwrap();
        assert!(matches!(&p.stmts[1], Stmt::If(_, t, e) if t.len() == 1 && e.len() == 1));
        let js = to_js(&p);
        assert!(js.contains("if((n>2.0))"), "{js}");
        assert!(js.contains("else{"), "{js}");
    }

    #[test]
    fn builtins_transpile_to_math() {
        let js = to_js(&parse("min(max(n, 0.0), abs(sqrt(floor(n))))").unwrap());
        assert!(
            js.contains("Math.min(Math.max(n,0.0),Math.abs(Math.sqrt(Math.floor(n))))"),
            "{js}"
        );
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

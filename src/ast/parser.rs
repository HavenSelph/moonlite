use crate::ast::lexer::{Base, Lexer, LexerIterator};
use crate::ast::span::Span;
use crate::ast::token::{Token, TokenKind};
use crate::ast::NodeKind::{BinaryOperation, UnaryOperation};
use crate::ast::{Node, NodeKind, Operator};
use crate::report::{Report, ReportKind, ReportLevel, ReportSender, Result, SpanToLabel};
use name_variant::NamedVariant;
use std::cmp::min;
use std::fmt::{Display, Formatter};
use ParserError::*;

#[derive(NamedVariant)]
enum ParserError {
    SyntaxError(String),
    UnexpectedEOF,
    UnexpectedToken(TokenKind),
}

impl Display for ParserError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.variant_name())?;
        match self {
            UnexpectedToken(kind) => write!(f, " {kind}")?,
            SyntaxError(msg) => write!(f, " {msg}")?,
            _ => (),
        }
        Ok(())
    }
}

impl ReportKind for ParserError {
    fn title(&self) -> String {
        format!("{}", self)
    }

    fn level(&self) -> ReportLevel {
        ReportLevel::Error
    }
}

pub struct Parser<'contents> {
    lexer: std::iter::Peekable<LexerIterator<'contents>>,
    current: Token<'contents>,
    reporter: ReportSender,
}

impl<'contents> Parser<'contents> {
    pub fn new(filename: &'static str, reporter: ReportSender) -> Result<Self> {
        let mut lexer = Lexer::new(filename)?.into_iter().peekable();
        let current = loop {
            match lexer.next() {
                Some(Err(report)) => reporter.report(report.finish().into()),
                Some(Ok(token)) => break token,
                _ => unreachable!(),
            }
        };
        Ok(Self {
            current,
            lexer,
            reporter,
        })
    }

    fn report(&self, report: Box<Report>) {
        self.reporter.report(report);
    }

    fn advance(&mut self) {
        self.current = loop {
            match self.lexer.next().expect("Advanced past EOF") {
                Err(report) => self.report(report.finish().into()),
                Ok(token) => break token,
            }
        }
    }

    fn skip_until<F: Fn(Token) -> bool>(&mut self, predicate: F) -> Option<Token> {
        loop {
            match self.current {
                token if predicate(token) => break Some(self.current.clone()),
                Token {
                    kind: TokenKind::EOF,
                    ..
                } => break None,
                _ => self.advance(),
            }
        }
    }

    fn sync<F: Fn(Token) -> bool>(&mut self, predicate: F) {
        self.skip_until(|token| /* Is it a new statement? */ matches!(token.kind,
                // This is where we will check for known statement beginners
                TokenKind::Semicolon
            ) || token.newline_before || predicate(token));
        if self.current.kind == TokenKind::Semicolon {
            self.advance();
        }
    }

    fn peek_is(&mut self, kind: TokenKind) -> bool {
        self.lexer
            .peek()
            .is_some_and(|result| result.as_ref().is_ok_and(|token| token.kind == kind))
    }

    fn consume<F: FnOnce(Token) -> bool, T: Display>(
        &mut self,
        predicate: F,
        message: T,
    ) -> Result<Token<'contents>> {
        match self.current {
            token if predicate(token) => {
                if token.kind != TokenKind::EOF {
                    self.advance();
                }
                Ok(token.clone())
            }
            token if token.kind == TokenKind::EOF => Err(UnexpectedEOF
                .make_labeled(token.span.labeled(message))
                .into()),
            token => Err(UnexpectedToken(token.kind)
                .make_labeled(token.span.labeled(message))
                .into()),
        }
    }

    fn consume_line(&mut self) -> Result<()> {
        match self.current {
            Token {
                kind: TokenKind::Semicolon,
                ..
            } => self.advance(),
            Token {
                kind: TokenKind::EOF,
                ..
            } => (),
            token if token.newline_before => (),
            token => {
                return Err(UnexpectedToken(token.kind)
                    .make_labeled(token.span.labeled("Expected end of statement"))
                    .into())
            }
        }
        Ok(())
    }

    fn consume_line_or(&mut self, expect: TokenKind) -> Result<()> {
        match self.current {
            Token {
                kind: TokenKind::Semicolon,
                ..
            } => self.advance(),
            Token {
                kind: TokenKind::EOF,
                ..
            } => (),
            token if token.newline_before || token.kind == expect => (),
            token => {
                return Err(UnexpectedToken(token.kind)
                    .make_labeled(
                        token
                            .span
                            .labeled(format!("Expected end of statement or {:?}", expect)),
                    )
                    .into())
            }
        }
        Ok(())
    }

    fn consume_one(&mut self, expect: TokenKind) -> Result<Token<'contents>> {
        self.consume(|token| token.kind == expect, format!("Expected {expect}"))
    }

    pub fn parse(&mut self) -> Box<Node> {
        self.parse_program()
    }

    fn parse_program(&mut self) -> Box<Node> {
        match self.parse_block(self.current.span, TokenKind::EOF) {
            Ok(val) => val,
            _ => panic!("Failed to parse global block."),
        }
    }

    fn parse_block(&mut self, start: Span, closer: TokenKind) -> Result<Box<Node>> {
        let mut stmts = Vec::new();
        let sync = |s: &mut Parser| s.sync(|token| token.kind == closer);

        while self.current.kind != closer && self.current.kind != TokenKind::EOF {
            match self.parse_statement() {
                Ok(stmt) => match self.consume_line_or(closer) {
                    Ok(_) => stmts.push(*stmt),
                    Err(e) => {
                        self.report(e.finish().into());
                        sync(self);
                    }
                },
                Err(e) => {
                    self.report(e.finish().into());
                    sync(self);
                }
            }
        }
        let end = self.consume_one(closer)?.span;

        Ok(NodeKind::Block(stmts).make(start.extend(end)).into())
    }

    fn parse_statement(&mut self) -> Result<Box<Node>> {
        self.parse_expression(0)
    }

    fn parse_expression(&mut self, min_bp: u8) -> Result<Box<Node>> {
        let mut lhs = match self.current.kind.as_prefix() {
            Some((op, _, rbp)) => {
                let span = self.current.span;
                self.advance();
                let rhs = self.parse_expression(rbp)?;
                let span = span.extend(rhs.span);
                UnaryOperation(op, rhs).make(span).into()
            }
            _ => self.parse_atom()?,
        };
        loop {
            let Some((op, lbp, rbp)) = self.current.kind.as_infix() else {
                break;
            };
            if lbp < min_bp {
                break;
            }
            self.advance();
            let rhs = self.parse_expression(rbp)?;
            let span = lhs.span.extend(rhs.span);
            lhs = BinaryOperation(op, lhs, rhs).make(span).into();
        }
        Ok(lhs)
    }

    fn parse_atom(&mut self) -> Result<Box<Node>> {
        let Token {
            kind, text, span, ..
        } = self.current;
        match kind {
            TokenKind::LeftParen => {
                self.advance();
                let mut expr = self.parse_expression(0)?;
                let end = self.consume_one(TokenKind::RightParen)?.span;
                expr.span = span.extend(end);
                Ok(expr)
            }
            TokenKind::Identifier => {
                self.advance();
                Ok(NodeKind::Identifier(text.to_string()).make(span).into())
            }
            TokenKind::BooleanLiteral => {
                self.advance();
                Ok(NodeKind::BooleanLiteral(text.eq("True")).make(span).into())
            }
            TokenKind::FloatLiteral => {
                self.advance();
                let val = text.parse().map_err(|err| {
                    SyntaxError("Invalid Float Literal".to_string())
                        .make_labeled(span.label())
                        .with_note(err)
                })?;
                Ok(NodeKind::FloatLiteral(val).make(span).into())
            }
            TokenKind::IntegerLiteralBin
            | TokenKind::IntegerLiteralDec
            | TokenKind::IntegerLiteralHex
            | TokenKind::IntegerLiteralOct => {
                let Token { kind, .. } = self.current;
                self.advance();
                let (base, radix) = match kind {
                    TokenKind::IntegerLiteralBin => (Base::Binary, 2),
                    TokenKind::IntegerLiteralOct => (Base::Octal, 8),
                    TokenKind::IntegerLiteralDec => (Base::Decimal, 10),
                    TokenKind::IntegerLiteralHex => (Base::Hexadecimal, 16),
                    _ => unreachable!(),
                };
                let val = usize::from_str_radix(text, radix).map_err(|err| {
                    Box::new(
                        SyntaxError(format!("Invalid {base:?} Integer literal"))
                            .make_labeled(span.label())
                            .with_note(err),
                    )
                })?;
                Ok(NodeKind::IntegerLiteral(val).make(span).into())
            }
            TokenKind::EOF => Err(UnexpectedEOF
                .make_labeled(span.labeled("Expected an expression"))
                .into()),
            _ => {
                self.advance();
                Err(UnexpectedToken(kind).make_labeled(span.label()).into())
            }
        }
    }
}

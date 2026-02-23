use std::collections::BTreeMap;

use crate::ast::{BinaryOp, ComparisonOp, Expr, Program, Statement, Variable};
use crate::error::{CompilerError, CompilerResult};
use crate::lexer::{tokenize, Span, Token, TokenKind};

pub fn parse_source(source: &str) -> CompilerResult<Program> {
	let tokens = tokenize(source)?;
	parse_tokens(tokens)
}

pub fn parse_tokens(tokens: Vec<Token>) -> CompilerResult<Program> {
	Parser::new(tokens).parse_program()
}

struct Parser {
	tokens: Vec<Token>,
	index: usize,
}

impl Parser {
	fn new(tokens: Vec<Token>) -> Self {
		Self { tokens, index: 0 }
	}

	fn parse_program(&mut self) -> CompilerResult<Program> {
		let mut lines: BTreeMap<u32, Statement> = BTreeMap::new();

		while !self.is_eof() {
			self.consume_newlines();
			if self.is_eof() {
				break;
			}

			let line_number = self.parse_line_number()?;
			let statement = self.parse_statement()?;

			if lines.insert(line_number, statement).is_some() {
				return Err(CompilerError::Parse(format!(
					"Duplicate line number {line_number}"
				)));
			}

			self.expect_newline_or_eof()?;
		}

		Ok(Program::new(lines))
	}

	fn parse_line_number(&mut self) -> CompilerResult<u32> {
		match self.next_kind() {
			TokenKind::LineNumber(value) => Ok(value),
			other => Err(self.error_current(format!(
				"Expected line number at start of line, found {:?}",
				other
			))),
		}
	}

	fn parse_statement(&mut self) -> CompilerResult<Statement> {
		match self.peek_kind() {
			TokenKind::Let => self.parse_let_statement(),
			TokenKind::Print => self.parse_print_statement(),
			TokenKind::Goto => self.parse_goto_statement(),
			TokenKind::If => self.parse_if_statement(),
			TokenKind::End => {
				self.advance();
				Ok(Statement::End)
			}
			TokenKind::Rem => {
				self.advance();
				Ok(Statement::Rem)
			}
			other => Err(self.error_current(format!(
				"Expected statement after line number, found {:?}",
				other
			))),
		}
	}

	fn parse_let_statement(&mut self) -> CompilerResult<Statement> {
		self.expect_token(TokenKind::Let)?;
		let var = self.parse_variable()?;
		self.expect_token(TokenKind::Eq)?;
		let expr = self.parse_expression()?;
		Ok(Statement::Let { var, expr })
	}

	fn parse_print_statement(&mut self) -> CompilerResult<Statement> {
		self.expect_token(TokenKind::Print)?;
		let mut items = Vec::new();
		items.push(self.parse_expression()?);

		while self.matches(TokenKind::Comma) {
			items.push(self.parse_expression()?);
		}

		Ok(Statement::Print { items })
	}

	fn parse_goto_statement(&mut self) -> CompilerResult<Statement> {
		self.expect_token(TokenKind::Goto)?;
		let target = self.parse_target_line_number()?;
		Ok(Statement::Goto { target })
	}

	fn parse_if_statement(&mut self) -> CompilerResult<Statement> {
		self.expect_token(TokenKind::If)?;
		let left = self.parse_expression()?;
		let op = self.parse_comparison_operator()?;
		let right = self.parse_expression()?;
		self.expect_token(TokenKind::Then)?;
		let target = self.parse_target_line_number()?;
		Ok(Statement::IfThen {
			left,
			op,
			right,
			target,
		})
	}

	fn parse_comparison_operator(&mut self) -> CompilerResult<ComparisonOp> {
		let op = match self.next_kind() {
			TokenKind::Eq => ComparisonOp::Eq,
			TokenKind::Ne => ComparisonOp::Ne,
			TokenKind::Lt => ComparisonOp::Lt,
			TokenKind::Gt => ComparisonOp::Gt,
			TokenKind::Le => ComparisonOp::Le,
			TokenKind::Ge => ComparisonOp::Ge,
			other => {
				return Err(self.error_current(format!(
					"Expected comparison operator, found {:?}",
					other
				)))
			}
		};
		Ok(op)
	}

	fn parse_target_line_number(&mut self) -> CompilerResult<u32> {
		match self.next_kind() {
			TokenKind::Integer(value) => u32::try_from(value).map_err(|_| {
				self.error_current(format!("Invalid jump target line number: {value}"))
			}),
			TokenKind::LineNumber(value) => Ok(value),
			other => Err(self.error_current(format!(
				"Expected target line number, found {:?}",
				other
			))),
		}
	}

	fn parse_variable(&mut self) -> CompilerResult<Variable> {
		match self.next_kind() {
			TokenKind::Variable(value) => Ok(value),
			other => Err(self.error_current(format!(
				"Expected variable A-Z, found {:?}",
				other
			))),
		}
	}

	fn parse_expression(&mut self) -> CompilerResult<Expr> {
		self.parse_add_sub()
	}

	fn parse_add_sub(&mut self) -> CompilerResult<Expr> {
		let mut left = self.parse_mul_div()?;

		loop {
			let op = match self.peek_kind() {
				TokenKind::Plus => BinaryOp::Add,
				TokenKind::Minus => BinaryOp::Sub,
				_ => break,
			};

			self.advance();
			let right = self.parse_mul_div()?;
			left = Expr::Binary {
				op,
				left: Box::new(left),
				right: Box::new(right),
			};
		}

		Ok(left)
	}

	fn parse_mul_div(&mut self) -> CompilerResult<Expr> {
		let mut left = self.parse_unary()?;

		loop {
			let op = match self.peek_kind() {
				TokenKind::Star => BinaryOp::Mul,
				TokenKind::Slash => BinaryOp::Div,
				_ => break,
			};

			self.advance();
			let right = self.parse_unary()?;
			left = Expr::Binary {
				op,
				left: Box::new(left),
				right: Box::new(right),
			};
		}

		Ok(left)
	}

	fn parse_unary(&mut self) -> CompilerResult<Expr> {
		if self.matches(TokenKind::Minus) {
			let right = self.parse_unary()?;
			return Ok(Expr::Binary {
				op: BinaryOp::Sub,
				left: Box::new(Expr::Int(0)),
				right: Box::new(right),
			});
		}

		self.parse_primary()
	}

	fn parse_primary(&mut self) -> CompilerResult<Expr> {
		match self.next_kind() {
			TokenKind::Integer(value) => Ok(Expr::Int(value)),
			TokenKind::Variable(value) => Ok(Expr::Var(value)),
			TokenKind::LParen => {
				let expr = self.parse_expression()?;
				self.expect_token(TokenKind::RParen)?;
				Ok(expr)
			}
			other => Err(self.error_current(format!(
				"Expected expression, found {:?}",
				other
			))),
		}
	}

	fn expect_newline_or_eof(&mut self) -> CompilerResult<()> {
		if self.matches(TokenKind::Newline) || self.is_eof() {
			return Ok(());
		}

		Err(self.error_current(format!(
			"Expected newline after statement, found {:?}",
			self.peek_kind()
		)))
	}

	fn expect_token(&mut self, expected: TokenKind) -> CompilerResult<()> {
		let got = self.next_kind();
		if got == expected {
			return Ok(());
		}

		Err(self.error_current(format!(
			"Expected {:?}, found {:?}",
			expected, got
		)))
	}

	fn matches(&mut self, expected: TokenKind) -> bool {
		if self.peek_kind() == expected {
			self.advance();
			true
		} else {
			false
		}
	}

	fn consume_newlines(&mut self) {
		while self.peek_kind() == TokenKind::Newline {
			self.advance();
		}
	}

	fn is_eof(&self) -> bool {
		self.peek_kind() == TokenKind::Eof
	}

	fn next_kind(&mut self) -> TokenKind {
		let kind = self.peek_kind();
		if !self.is_eof() {
			self.index += 1;
		}
		kind
	}

	fn advance(&mut self) {
		if !self.is_eof() {
			self.index += 1;
		}
	}

	fn peek_kind(&self) -> TokenKind {
		self.tokens
			.get(self.index)
			.map(|token| token.kind.clone())
			.unwrap_or(TokenKind::Eof)
	}

	fn current_span(&self) -> Option<Span> {
		self.tokens.get(self.index).map(|token| token.span)
	}

	fn error_current(&self, message: String) -> CompilerError {
		if let Some(span) = self.current_span() {
			CompilerError::Parse(format!(
				"{} at {}:{}",
				message, span.start.line, span.start.column
			))
		} else {
			CompilerError::Parse(message)
		}
	}
}

#[cfg(test)]
mod tests {
	use crate::ast::{BinaryOp, Expr, Statement, Variable};
	use crate::parser::parse_source;

	#[test]
	fn parses_expression_precedence_in_let() {
		let program = parse_source("10 LET A = 1 + 2 * 3\n20 END\n").expect("parse should pass");
		let stmt = program.lines.get(&10).expect("line 10 expected");
		match stmt {
			Statement::Let { var, expr } => {
				assert_eq!(*var, Variable(0));
				assert_eq!(
					expr,
					&Expr::Binary {
						op: BinaryOp::Add,
						left: Box::new(Expr::Int(1)),
						right: Box::new(Expr::Binary {
							op: BinaryOp::Mul,
							left: Box::new(Expr::Int(2)),
							right: Box::new(Expr::Int(3)),
						}),
					}
				);
			}
			_ => panic!("expected LET statement"),
		}
	}

	#[test]
	fn parses_if_then_statement() {
		let program = parse_source("10 IF A <= 10 THEN 40\n20 END\n").expect("parse should pass");
		let stmt = program.lines.get(&10).expect("line 10 expected");
		match stmt {
			Statement::IfThen {
				left,
				op,
				right,
				target,
			} => {
				assert_eq!(*left, Expr::Var(Variable(0)));
				assert_eq!(*op, crate::ast::ComparisonOp::Le);
				assert_eq!(*right, Expr::Int(10));
				assert_eq!(*target, 40);
			}
			_ => panic!("expected IF THEN statement"),
		}
	}

	#[test]
	fn rejects_duplicate_line_numbers() {
		let result = parse_source("10 END\n10 END\n");
		assert!(result.is_err());
		let message = format!("{}", result.expect_err("should fail"));
		assert!(message.contains("Duplicate line number 10"));
	}
}

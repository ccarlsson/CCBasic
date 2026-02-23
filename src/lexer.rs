use crate::ast::Variable;
use crate::error::{CompilerError, CompilerResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePos {
	pub line: u32,
	pub column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
	pub start: SourcePos,
	pub end: SourcePos,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
	LineNumber(u32),
	Integer(i64),
	Variable(Variable),
	Let,
	Print,
	Goto,
	If,
	Then,
	End,
	Rem,
	Plus,
	Minus,
	Star,
	Slash,
	Eq,
	Ne,
	Lt,
	Gt,
	Le,
	Ge,
	LParen,
	RParen,
	Comma,
	Newline,
	Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
	pub kind: TokenKind,
	pub span: Span,
}

pub fn tokenize(source: &str) -> CompilerResult<Vec<Token>> {
	Lexer::new(source).tokenize()
}

struct Lexer {
	chars: Vec<char>,
	index: usize,
	line: u32,
	column: u32,
	at_line_start: bool,
}

impl Lexer {
	fn new(source: &str) -> Self {
		Self {
			chars: source.chars().collect(),
			index: 0,
			line: 1,
			column: 1,
			at_line_start: true,
		}
	}

	fn tokenize(mut self) -> CompilerResult<Vec<Token>> {
		let mut tokens = Vec::new();

		while let Some(ch) = self.peek() {
			match ch {
				' ' | '\t' => {
					self.advance();
				}
				'\r' => {
					self.advance();
				}
				'\n' => {
					let (_, pos) = self.advance().expect("peeked char must advance");
					tokens.push(Token {
						kind: TokenKind::Newline,
						span: Span {
							start: pos,
							end: pos,
						},
					});
					self.at_line_start = true;
				}
				'0'..='9' => {
					tokens.push(self.lex_number()?);
					self.at_line_start = false;
				}
				'A'..='Z' | 'a'..='z' => {
					let token = self.lex_word()?;
					let is_rem = matches!(token.kind, TokenKind::Rem);
					tokens.push(token);
					self.at_line_start = false;
					if is_rem {
						self.consume_until_newline();
					}
				}
				'+' => {
					tokens.push(self.lex_single(TokenKind::Plus));
					self.at_line_start = false;
				}
				'-' => {
					tokens.push(self.lex_single(TokenKind::Minus));
					self.at_line_start = false;
				}
				'*' => {
					tokens.push(self.lex_single(TokenKind::Star));
					self.at_line_start = false;
				}
				'/' => {
					tokens.push(self.lex_single(TokenKind::Slash));
					self.at_line_start = false;
				}
				'(' => {
					tokens.push(self.lex_single(TokenKind::LParen));
					self.at_line_start = false;
				}
				')' => {
					tokens.push(self.lex_single(TokenKind::RParen));
					self.at_line_start = false;
				}
				',' => {
					tokens.push(self.lex_single(TokenKind::Comma));
					self.at_line_start = false;
				}
				'=' => {
					tokens.push(self.lex_single(TokenKind::Eq));
					self.at_line_start = false;
				}
				'<' => {
					tokens.push(self.lex_less_family());
					self.at_line_start = false;
				}
				'>' => {
					tokens.push(self.lex_greater_family());
					self.at_line_start = false;
				}
				_ => {
					return Err(CompilerError::Lex(format!(
						"Unexpected character '{}' at {}:{}",
						ch, self.line, self.column
					)));
				}
			}
		}

		let pos = SourcePos {
			line: self.line,
			column: self.column,
		};
		tokens.push(Token {
			kind: TokenKind::Eof,
			span: Span {
				start: pos,
				end: pos,
			},
		});

		Ok(tokens)
	}

	fn lex_single(&mut self, kind: TokenKind) -> Token {
		let (_, pos) = self.advance().expect("single token must advance");
		Token {
			kind,
			span: Span {
				start: pos,
				end: pos,
			},
		}
	}

	fn lex_less_family(&mut self) -> Token {
		let (_, start) = self.advance().expect("less family must advance");
		if let Some((_, end)) = self.match_char('=') {
			return Token {
				kind: TokenKind::Le,
				span: Span { start, end },
			};
		}
		if let Some((_, end)) = self.match_char('>') {
			return Token {
				kind: TokenKind::Ne,
				span: Span { start, end },
			};
		}
		Token {
			kind: TokenKind::Lt,
			span: Span { start, end: start },
		}
	}

	fn lex_greater_family(&mut self) -> Token {
		let (_, start) = self.advance().expect("greater family must advance");
		if let Some((_, end)) = self.match_char('=') {
			return Token {
				kind: TokenKind::Ge,
				span: Span { start, end },
			};
		}
		Token {
			kind: TokenKind::Gt,
			span: Span { start, end: start },
		}
	}

	fn lex_number(&mut self) -> CompilerResult<Token> {
		let (text, start, end) = self.consume_while(|value| value.is_ascii_digit());
		if self.at_line_start {
			let value = text.parse::<u32>().map_err(|_| {
				CompilerError::Lex(format!(
					"Invalid line number '{}' at {}:{}",
					text, start.line, start.column
				))
			})?;
			return Ok(Token {
				kind: TokenKind::LineNumber(value),
				span: Span { start, end },
			});
		}

		let value = text.parse::<i64>().map_err(|_| {
			CompilerError::Lex(format!(
				"Invalid integer literal '{}' at {}:{}",
				text, start.line, start.column
			))
		})?;

		Ok(Token {
			kind: TokenKind::Integer(value),
			span: Span { start, end },
		})
	}

	fn lex_word(&mut self) -> CompilerResult<Token> {
		let (text, start, end) = self.consume_while(|value| value.is_ascii_alphanumeric());
		let upper = text.to_ascii_uppercase();
		let kind = match upper.as_str() {
			"LET" => TokenKind::Let,
			"PRINT" => TokenKind::Print,
			"GOTO" => TokenKind::Goto,
			"IF" => TokenKind::If,
			"THEN" => TokenKind::Then,
			"END" => TokenKind::End,
			"REM" => TokenKind::Rem,
			_ if upper.len() == 1 => {
				let variable = Variable::from_ascii_letter(upper.chars().next().unwrap()).ok_or_else(
					|| {
						CompilerError::Lex(format!(
							"Invalid variable '{}' at {}:{}",
							text, start.line, start.column
						))
					},
				)?;
				TokenKind::Variable(variable)
			}
			_ => {
				return Err(CompilerError::Lex(format!(
					"Unknown identifier '{}' at {}:{}",
					text, start.line, start.column
				)));
			}
		};

		Ok(Token {
			kind,
			span: Span { start, end },
		})
	}

	fn consume_until_newline(&mut self) {
		while let Some(value) = self.peek() {
			if value == '\n' {
				break;
			}
			self.advance();
		}
	}

	fn consume_while(&mut self, predicate: impl Fn(char) -> bool) -> (String, SourcePos, SourcePos) {
		let mut text = String::new();
		let mut start: Option<SourcePos> = None;
		let mut end: Option<SourcePos> = None;

		while let Some(value) = self.peek() {
			if !predicate(value) {
				break;
			}
			let (ch, pos) = self.advance().expect("peeked character must advance");
			if start.is_none() {
				start = Some(pos);
			}
			end = Some(pos);
			text.push(ch);
		}

		(
			text,
			start.expect("consume_while requires at least one character"),
			end.expect("consume_while requires at least one character"),
		)
	}

	fn peek(&self) -> Option<char> {
		self.chars.get(self.index).copied()
	}

	fn match_char(&mut self, target: char) -> Option<(char, SourcePos)> {
		if self.peek()? == target {
			self.advance()
		} else {
			None
		}
	}

	fn advance(&mut self) -> Option<(char, SourcePos)> {
		let ch = self.peek()?;
		let pos = SourcePos {
			line: self.line,
			column: self.column,
		};
		self.index += 1;
		if ch == '\n' {
			self.line += 1;
			self.column = 1;
		} else {
			self.column += 1;
		}
		Some((ch, pos))
	}
}

#[cfg(test)]
mod tests {
	use super::{tokenize, TokenKind};
	use crate::ast::Variable;

	fn kinds(source: &str) -> Vec<TokenKind> {
		tokenize(source)
			.expect("tokenization should succeed")
			.into_iter()
			.map(|token| token.kind)
			.collect()
	}

	#[test]
	fn tokenizes_case_insensitive_keywords_and_variables() {
		let values = kinds("10 let a = 1\n20 PrInT A\n30 END\n");
		assert_eq!(
			values,
			vec![
				TokenKind::LineNumber(10),
				TokenKind::Let,
				TokenKind::Variable(Variable(0)),
				TokenKind::Eq,
				TokenKind::Integer(1),
				TokenKind::Newline,
				TokenKind::LineNumber(20),
				TokenKind::Print,
				TokenKind::Variable(Variable(0)),
				TokenKind::Newline,
				TokenKind::LineNumber(30),
				TokenKind::End,
				TokenKind::Newline,
				TokenKind::Eof,
			]
		);
	}

	#[test]
	fn tokenizes_comparison_operators() {
		let values = kinds("10 IF A <= 10 THEN 40\n20 IF A <> 5 THEN 30\n");
		assert_eq!(
			values,
			vec![
				TokenKind::LineNumber(10),
				TokenKind::If,
				TokenKind::Variable(Variable(0)),
				TokenKind::Le,
				TokenKind::Integer(10),
				TokenKind::Then,
				TokenKind::Integer(40),
				TokenKind::Newline,
				TokenKind::LineNumber(20),
				TokenKind::If,
				TokenKind::Variable(Variable(0)),
				TokenKind::Ne,
				TokenKind::Integer(5),
				TokenKind::Then,
				TokenKind::Integer(30),
				TokenKind::Newline,
				TokenKind::Eof,
			]
		);
	}

	#[test]
	fn rem_consumes_remainder_of_line() {
		let values = kinds("10 REM ignore + 123 <= A\n20 PRINT A\n");
		assert_eq!(
			values,
			vec![
				TokenKind::LineNumber(10),
				TokenKind::Rem,
				TokenKind::Newline,
				TokenKind::LineNumber(20),
				TokenKind::Print,
				TokenKind::Variable(Variable(0)),
				TokenKind::Newline,
				TokenKind::Eof,
			]
		);
	}
}

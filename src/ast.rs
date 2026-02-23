use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Program {
	pub lines: BTreeMap<u32, Statement>,
}

impl Program {
	pub fn new(lines: BTreeMap<u32, Statement>) -> Self {
		Self { lines }
	}

	pub fn is_empty(&self) -> bool {
		self.lines.is_empty()
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
	Let { var: Variable, expr: Expr },
	Print { items: Vec<Expr> },
	Goto { target: u32 },
	IfThen {
		left: Expr,
		op: ComparisonOp,
		right: Expr,
		target: u32,
	},
	End,
	Rem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
	Int(i64),
	Var(Variable),
	Binary {
		op: BinaryOp,
		left: Box<Expr>,
		right: Box<Expr>,
	},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Variable(pub u8);

impl Variable {
	pub const MIN: u8 = 0;
	pub const MAX: u8 = 25;

	pub fn from_ascii_letter(value: char) -> Option<Self> {
		let upper = value.to_ascii_uppercase();
		if !upper.is_ascii_uppercase() {
			return None;
		}

		let index = (upper as u8).checked_sub(b'A')?;
		if index <= Self::MAX {
			Some(Self(index))
		} else {
			None
		}
	}

	pub fn as_ascii_letter(self) -> char {
		char::from(b'A' + self.0)
	}

	pub fn index(self) -> usize {
		usize::from(self.0)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
	Add,
	Sub,
	Mul,
	Div,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOp {
	Eq,
	Ne,
	Lt,
	Gt,
	Le,
	Ge,
}

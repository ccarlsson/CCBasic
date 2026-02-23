use crate::ast::{BinaryOp, Expr, Program, Statement, StrVariable, Variable};
use crate::error::{CompilerError, CompilerResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExprType {
	Int,
	Str,
}

pub fn analyze(program: &Program) -> CompilerResult<()> {
	let line_numbers: Vec<u32> = program.lines.keys().copied().collect();
	if line_numbers.is_empty() {
		return Ok(());
	}

	let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); line_numbers.len()];
	let mut reads_per_line: Vec<Vec<VarRead>> = vec![Vec::new(); line_numbers.len()];
	let mut defs_per_line: Vec<Option<VarDef>> = vec![None; line_numbers.len()];

	for (index, line_number) in line_numbers.iter().enumerate() {
		let statement = program.lines.get(line_number).ok_or_else(|| {
			CompilerError::Semantic(format!("Internal error: missing line {line_number}"))
		})?;

		validate_statement_types(statement, *line_number)?;
		reads_per_line[index] = statement_reads(statement);
		defs_per_line[index] = statement_def(statement);

		for successor in successors(statement, index, &line_numbers, program)? {
			predecessors[successor].push(index);
		}
	}

	let mut in_sets = vec![VarInitSet::new_uninitialized(); line_numbers.len()];
	let mut out_sets = vec![VarInitSet::new_uninitialized(); line_numbers.len()];

	loop {
		let mut changed = false;
		for index in 0..line_numbers.len() {
			let new_in = if predecessors[index].is_empty() {
				VarInitSet::new_uninitialized()
			} else {
				intersect_predecessors(&predecessors[index], &out_sets)
			};

			let mut new_out = new_in;
			if let Some(def) = defs_per_line[index] {
				new_out.mark_defined(def);
			}

			if new_in != in_sets[index] || new_out != out_sets[index] {
				in_sets[index] = new_in;
				out_sets[index] = new_out;
				changed = true;
			}
		}

		if !changed {
			break;
		}
	}

	for (index, line_number) in line_numbers.iter().enumerate() {
		for read in &reads_per_line[index] {
			if !in_sets[index].is_defined(*read) {
				return Err(CompilerError::Semantic(format!(
					"Variable {} used before initialization at line {}",
					read.display_name(),
					line_number
				)));
			}
		}
	}

	Ok(())
}

pub fn infer_expr_type(expr: &Expr) -> CompilerResult<ExprType> {
	match expr {
		Expr::Int(_) | Expr::Var(_) => Ok(ExprType::Int),
		Expr::StrLit(_) | Expr::StrVar(_) => Ok(ExprType::Str),
		Expr::Binary { op, left, right } => {
			let left_type = infer_expr_type(left)?;
			let right_type = infer_expr_type(right)?;
			match op {
				BinaryOp::Mul | BinaryOp::Div | BinaryOp::Sub => {
					if left_type == ExprType::Int && right_type == ExprType::Int {
						Ok(ExprType::Int)
					} else {
						Err(CompilerError::Semantic(format!(
							"Operator {:?} requires integer operands",
							op
						)))
					}
				}
				BinaryOp::Add => {
					if left_type == ExprType::Int && right_type == ExprType::Int {
						Ok(ExprType::Int)
					} else if left_type == ExprType::Str && right_type == ExprType::Str {
						Ok(ExprType::Str)
					} else {
						Err(CompilerError::Semantic(
							"Operator Add requires both operands to be int or both to be string"
								.to_string(),
						))
					}
				}
			}
		}
	}
}

fn validate_statement_types(statement: &Statement, line_number: u32) -> CompilerResult<()> {
	match statement {
		Statement::Let { expr, .. } => {
			ensure_expr_type(expr, ExprType::Int, line_number, "LET")?;
		}
		Statement::LetStr { expr, .. } => {
			ensure_expr_type(expr, ExprType::Str, line_number, "LET")?;
		}
		Statement::Print { items } => {
			for item in items {
				infer_expr_type(item).map_err(|error| {
					CompilerError::Semantic(format!(
						"{} at line {}",
						error,
						line_number
					))
				})?;
			}
		}
		Statement::IfThen { left, right, .. } => {
			ensure_expr_type(left, ExprType::Int, line_number, "IF")?;
			ensure_expr_type(right, ExprType::Int, line_number, "IF")?;
		}
		Statement::InputInt { .. }
		| Statement::InputStr { .. }
		| Statement::Goto { .. }
		| Statement::End
		| Statement::Rem => {}
	}
	Ok(())
}

fn ensure_expr_type(
	expr: &Expr,
	expected: ExprType,
	line_number: u32,
	context: &str,
) -> CompilerResult<()> {
	let actual = infer_expr_type(expr)?;
	if actual == expected {
		return Ok(());
	}

	Err(CompilerError::Semantic(format!(
		"{context} at line {line_number} expects {:?} expression but got {:?}",
		expected, actual
	)))
}

fn successors(
	statement: &Statement,
	index: usize,
	line_numbers: &[u32],
	program: &Program,
) -> CompilerResult<Vec<usize>> {
	match statement {
		Statement::End => Ok(Vec::new()),
		Statement::Goto { target } => Ok(vec![line_index(*target, line_numbers, program)?]),
		Statement::IfThen { target, .. } => {
			let mut values = vec![line_index(*target, line_numbers, program)?];
			if let Some(next) = next_index(index, line_numbers) {
				if !values.contains(&next) {
					values.push(next);
				}
			}
			Ok(values)
		}
		Statement::Let { .. }
		| Statement::LetStr { .. }
		| Statement::InputInt { .. }
		| Statement::InputStr { .. }
		| Statement::Print { .. }
		| Statement::Rem => Ok(next_index(index, line_numbers).into_iter().collect()),
	}
}

fn line_index(target: u32, line_numbers: &[u32], program: &Program) -> CompilerResult<usize> {
	if !program.lines.contains_key(&target) {
		return Err(CompilerError::Semantic(format!(
			"Branch target line {} does not exist",
			target
		)));
	}

	line_numbers
		.iter()
		.position(|line| *line == target)
		.ok_or_else(|| CompilerError::Semantic(format!("Internal error: missing line {target}")))
}

fn next_index(index: usize, line_numbers: &[u32]) -> Option<usize> {
	if index + 1 < line_numbers.len() {
		Some(index + 1)
	} else {
		None
	}
}

fn intersect_predecessors(predecessors: &[usize], out_sets: &[VarInitSet]) -> VarInitSet {
	let mut result = VarInitSet::new_initialized();
	for predecessor in predecessors {
		result.intersect_with(out_sets[*predecessor]);
	}
	result
}

fn statement_reads(statement: &Statement) -> Vec<VarRead> {
	let mut reads = Vec::new();
	match statement {
		Statement::Let { expr, .. } | Statement::LetStr { expr, .. } => {
			collect_expr_reads(expr, &mut reads)
		}
		Statement::Print { items } => {
			for item in items {
				collect_expr_reads(item, &mut reads);
			}
		}
		Statement::IfThen { left, right, .. } => {
			collect_expr_reads(left, &mut reads);
			collect_expr_reads(right, &mut reads);
		}
		Statement::InputInt { .. }
		| Statement::InputStr { .. }
		| Statement::Goto { .. }
		| Statement::End
		| Statement::Rem => {}
	}
	reads
}

fn statement_def(statement: &Statement) -> Option<VarDef> {
	match statement {
		Statement::Let { var, .. } | Statement::InputInt { var } => Some(VarDef::Int(*var)),
		Statement::LetStr { var, .. } | Statement::InputStr { var } => Some(VarDef::Str(*var)),
		_ => None,
	}
}

fn collect_expr_reads(expr: &Expr, out: &mut Vec<VarRead>) {
	match expr {
		Expr::Int(_) | Expr::StrLit(_) => {}
		Expr::Var(var) => out.push(VarRead::Int(*var)),
		Expr::StrVar(var) => out.push(VarRead::Str(*var)),
		Expr::Binary { left, right, .. } => {
			collect_expr_reads(left, out);
			collect_expr_reads(right, out);
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VarInitSet {
	ints: [bool; 26],
	strings: [bool; 26],
}

impl VarInitSet {
	fn new_uninitialized() -> Self {
		Self {
			ints: [false; 26],
			strings: [false; 26],
		}
	}

	fn new_initialized() -> Self {
		Self {
			ints: [true; 26],
			strings: [true; 26],
		}
	}

	fn mark_defined(&mut self, var: VarDef) {
		match var {
			VarDef::Int(value) => self.ints[value.index()] = true,
			VarDef::Str(value) => self.strings[value.index()] = true,
		}
	}

	fn is_defined(&self, var: VarRead) -> bool {
		match var {
			VarRead::Int(value) => self.ints[value.index()],
			VarRead::Str(value) => self.strings[value.index()],
		}
	}

	fn intersect_with(&mut self, other: Self) {
		for (slot, value) in self.ints.iter_mut().enumerate() {
			*value &= other.ints[slot];
		}
		for (slot, value) in self.strings.iter_mut().enumerate() {
			*value &= other.strings[slot];
		}
	}
}

#[derive(Debug, Clone, Copy)]
enum VarRead {
	Int(Variable),
	Str(StrVariable),
}

impl VarRead {
	fn display_name(self) -> String {
		match self {
			Self::Int(var) => var.as_ascii_letter().to_string(),
			Self::Str(var) => format!("{}$", var.as_ascii_letter()),
		}
	}
}

#[derive(Debug, Clone, Copy)]
enum VarDef {
	Int(Variable),
	Str(StrVariable),
}

#[cfg(test)]
mod tests {
	use crate::parser::parse_source;
	use crate::semantic::analyze;

	#[test]
	fn rejects_missing_branch_target() {
		let program = parse_source("10 GOTO 99\n20 END\n").expect("parse should pass");
		let result = analyze(&program);
		assert!(result.is_err());
		assert!(format!("{}", result.expect_err("must fail")).contains("line 99"));
	}

	#[test]
	fn rejects_uninitialized_variable_use() {
		let program = parse_source("10 PRINT A\n20 END\n").expect("parse should pass");
		let result = analyze(&program);
		assert!(result.is_err());
		assert!(
			format!("{}", result.expect_err("must fail"))
				.contains("Variable A used before initialization")
		);
	}

	#[test]
	fn accepts_initialized_variable_use() {
		let program =
			parse_source("10 LET A = 1\n20 PRINT A\n30 END\n").expect("parse should pass");
		analyze(&program).expect("semantic analysis should pass");
	}

	#[test]
	fn rejects_branch_path_with_uninitialized_variable() {
		let program = parse_source("10 IF 1 = 1 THEN 30\n20 LET B = 2\n30 PRINT B\n40 END\n")
			.expect("parse should pass");
		let result = analyze(&program);
		assert!(result.is_err());
		assert!(
			format!("{}", result.expect_err("must fail"))
				.contains("Variable B used before initialization")
		);
	}

	#[test]
	fn accepts_variable_initialized_on_all_paths() {
		let program = parse_source(
			"10 LET B = 1\n20 IF 1 = 1 THEN 40\n30 LET B = 2\n40 PRINT B\n50 END\n",
		)
		.expect("parse should pass");
		analyze(&program).expect("semantic analysis should pass");
	}

	#[test]
	fn rejects_string_use_before_initialization() {
		let program = parse_source("10 PRINT A$\n20 END\n").expect("parse should pass");
		let result = analyze(&program);
		assert!(result.is_err());
		assert!(
			format!("{}", result.expect_err("must fail"))
				.contains("Variable A$ used before initialization")
		);
	}

	#[test]
	fn rejects_mixed_string_and_integer_addition() {
		let program = parse_source("10 LET A = 1 + \"X\"\n20 END\n").expect("parse should pass");
		let result = analyze(&program);
		assert!(result.is_err());
		assert!(format!("{}", result.expect_err("must fail")).contains("Operator Add"));
	}
}

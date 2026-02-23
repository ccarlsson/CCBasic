use crate::ast::{Expr, Program, Statement, Variable};
use crate::error::{CompilerError, CompilerResult};

pub fn analyze(program: &Program) -> CompilerResult<()> {
	let line_numbers: Vec<u32> = program.lines.keys().copied().collect();
	if line_numbers.is_empty() {
		return Ok(());
	}

	let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); line_numbers.len()];
	let mut reads_per_line: Vec<Vec<Variable>> = vec![Vec::new(); line_numbers.len()];
	let mut defs_per_line: Vec<Option<Variable>> = vec![None; line_numbers.len()];

	for (index, line_number) in line_numbers.iter().enumerate() {
		let statement = program.lines.get(line_number).ok_or_else(|| {
			CompilerError::Semantic(format!("Internal error: missing line {line_number}"))
		})?;

		reads_per_line[index] = statement_reads(statement);
		defs_per_line[index] = statement_def(statement);

		for successor in successors(statement, index, &line_numbers, program)? {
			predecessors[successor].push(index);
		}
	}

	let mut in_sets = vec![[false; 26]; line_numbers.len()];
	let mut out_sets = vec![[false; 26]; line_numbers.len()];

	loop {
		let mut changed = false;
		for index in 0..line_numbers.len() {
			let new_in = if predecessors[index].is_empty() {
				[false; 26]
			} else {
				intersect_predecessors(&predecessors[index], &out_sets)
			};

			let mut new_out = new_in;
			if let Some(def) = defs_per_line[index] {
				new_out[def.index()] = true;
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
		for variable in &reads_per_line[index] {
			if !in_sets[index][variable.index()] {
				return Err(CompilerError::Semantic(format!(
					"Variable {} used before initialization at line {}",
					variable.as_ascii_letter(),
					line_number
				)));
			}
		}
	}

	Ok(())
}

fn successors(
	statement: &Statement,
	index: usize,
	line_numbers: &[u32],
	program: &Program,
) -> CompilerResult<Vec<usize>> {
	match statement {
		Statement::End => Ok(Vec::new()),
		Statement::Goto { target } => {
			Ok(vec![line_index(*target, line_numbers, program)?])
		}
		Statement::IfThen { target, .. } => {
			let mut values = vec![line_index(*target, line_numbers, program)?];
			if let Some(next) = next_index(index, line_numbers) {
				if !values.contains(&next) {
					values.push(next);
				}
			}
			Ok(values)
		}
		Statement::Let { .. } | Statement::Print { .. } | Statement::Rem => {
			Ok(next_index(index, line_numbers).into_iter().collect())
		}
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

fn intersect_predecessors(predecessors: &[usize], out_sets: &[[bool; 26]]) -> [bool; 26] {
	let mut result = [true; 26];
	for predecessor in predecessors {
		for (slot, value) in result.iter_mut().enumerate() {
			*value &= out_sets[*predecessor][slot];
		}
	}
	result
}

fn statement_reads(statement: &Statement) -> Vec<Variable> {
	let mut reads = Vec::new();
	match statement {
		Statement::Let { expr, .. } => collect_expr_reads(expr, &mut reads),
		Statement::Print { items } => {
			for item in items {
				collect_expr_reads(item, &mut reads);
			}
		}
		Statement::IfThen { left, right, .. } => {
			collect_expr_reads(left, &mut reads);
			collect_expr_reads(right, &mut reads);
		}
		Statement::Goto { .. } | Statement::End | Statement::Rem => {}
	}
	reads
}

fn statement_def(statement: &Statement) -> Option<Variable> {
	match statement {
		Statement::Let { var, .. } => Some(*var),
		_ => None,
	}
}

fn collect_expr_reads(expr: &Expr, out: &mut Vec<Variable>) {
	match expr {
		Expr::Int(_) => {}
		Expr::Var(var) => out.push(*var),
		Expr::Binary { left, right, .. } => {
			collect_expr_reads(left, out);
			collect_expr_reads(right, out);
		}
	}
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
			format!("{}", result.expect_err("must fail")).contains("Variable A used before initialization")
		);
	}

	#[test]
	fn accepts_initialized_variable_use() {
		let program = parse_source("10 LET A = 1\n20 PRINT A\n30 END\n")
			.expect("parse should pass");
		analyze(&program).expect("semantic analysis should pass");
	}

	#[test]
	fn rejects_branch_path_with_uninitialized_variable() {
		let program = parse_source("10 IF 1 = 1 THEN 30\n20 LET B = 2\n30 PRINT B\n40 END\n")
			.expect("parse should pass");
		let result = analyze(&program);
		assert!(result.is_err());
		assert!(
			format!("{}", result.expect_err("must fail")).contains("Variable B used before initialization")
		);
	}

	#[test]
	fn accepts_variable_initialized_on_all_paths() {
		let program = parse_source("10 LET B = 1\n20 IF 1 = 1 THEN 40\n30 LET B = 2\n40 PRINT B\n50 END\n")
			.expect("parse should pass");
		analyze(&program).expect("semantic analysis should pass");
	}
}

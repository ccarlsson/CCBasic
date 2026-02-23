use std::collections::BTreeSet;

use crate::ast::{BinaryOp, ComparisonOp, Expr, Program, Statement, StrVariable, Variable};
use crate::error::{CompilerError, CompilerResult};
use crate::semantic::{infer_expr_type, ExprType};

const STRING_HEAP_SIZE: usize = 65_536;
const INPUT_BUFFER_SIZE: usize = 4_096;

pub fn generate_assembly(program: &Program) -> CompilerResult<String> {
	let mut generator = Generator::new(program);
	generator.generate()?;
	Ok(generator.output)
}

struct Generator<'a> {
	program: &'a Program,
	line_numbers: Vec<u32>,
	line_set: BTreeSet<u32>,
	string_literals: Vec<String>,
	output: String,
}

impl<'a> Generator<'a> {
	fn new(program: &'a Program) -> Self {
		let line_numbers: Vec<u32> = program.lines.keys().copied().collect();
		let line_set = line_numbers.iter().copied().collect();
		let string_literals = collect_string_literals(program);
		Self {
			program,
			line_numbers,
			line_set,
			string_literals,
			output: String::new(),
		}
	}

	fn generate(&mut self) -> CompilerResult<()> {
		self.emit_preamble();
		self.emit_start();
		self.emit_lines()?;
		self.emit_runtime();
		self.emit_data();
		Ok(())
	}

	fn emit_preamble(&mut self) {
		self.push("global _start");
		self.push("section .text");
	}

	fn emit_start(&mut self) {
		self.push("_start:");
		self.push("    lea rax, [rel str_heap]");
		self.push("    mov [str_heap_ptr], rax");
		if let Some(first) = self.line_numbers.first() {
			self.push(&format!("    jmp {}", line_label(*first)));
		} else {
			self.push("    mov rax, 60");
			self.push("    xor rdi, rdi");
			self.push("    syscall");
		}
	}

	fn emit_lines(&mut self) -> CompilerResult<()> {
		for index in 0..self.line_numbers.len() {
			let line_number = self.line_numbers[index];
			let statement = self.program.lines.get(&line_number).cloned().ok_or_else(|| {
				CompilerError::Codegen(format!("Internal error: missing line {line_number}"))
			})?;
			let next = self.line_numbers.get(index + 1).copied();

			self.push(&format!("{}:", line_label(line_number)));
			match statement {
				Statement::Let { var, expr } => {
					self.emit_int_expr(&expr)?;
					self.push(&format!("    mov [var_{}], rax", var_name(var)));
					self.emit_fallthrough(next);
				}
				Statement::LetStr { var, expr } => {
					self.emit_str_expr(&expr)?;
					self.push(&format!("    mov [str_ptr_{}], rax", str_var_name(var)));
					self.push(&format!("    mov [str_len_{}], rdx", str_var_name(var)));
					self.emit_fallthrough(next);
				}
				Statement::Print { items } => {
					for (item_index, item) in items.iter().enumerate() {
						match infer_expr_type(item)? {
							ExprType::Int => {
								self.emit_int_expr(item)?;
								self.push("    call print_i64");
							}
							ExprType::Str => {
								self.emit_str_expr(item)?;
								self.push("    call print_str");
							}
						}
						if item_index + 1 != items.len() {
							self.push("    call print_space");
						}
					}
					self.push("    call print_newline");
					self.emit_fallthrough(next);
				}
				Statement::InputInt { var } => {
					self.push("    call input_i64");
					self.push(&format!("    mov [var_{}], rax", var_name(var)));
					self.emit_fallthrough(next);
				}
				Statement::InputStr { var } => {
					self.push("    call input_str");
					self.push(&format!("    mov [str_ptr_{}], rax", str_var_name(var)));
					self.push(&format!("    mov [str_len_{}], rdx", str_var_name(var)));
					self.emit_fallthrough(next);
				}
				Statement::Goto { target } => {
					self.ensure_target_exists(target)?;
					self.push(&format!("    jmp {}", line_label(target)));
				}
				Statement::IfThen {
					left,
					op,
					right,
					target,
				} => {
					self.ensure_target_exists(target)?;
					self.emit_int_expr(&left)?;
					self.push("    push rax");
					self.emit_int_expr(&right)?;
					self.push("    pop rbx");
					self.push("    cmp rbx, rax");
					self.push(&format!("    {} {}", comparison_jump(op), line_label(target)));
					self.emit_fallthrough(next);
				}
				Statement::End => {
					self.push("    mov rax, 60");
					self.push("    xor rdi, rdi");
					self.push("    syscall");
				}
				Statement::Rem => {
					self.emit_fallthrough(next);
				}
			}
		}

		Ok(())
	}

	fn emit_int_expr(&mut self, expr: &Expr) -> CompilerResult<()> {
		match expr {
			Expr::Int(value) => self.push(&format!("    mov rax, {value}")),
			Expr::Var(var) => self.push(&format!("    mov rax, [var_{}]", var_name(*var))),
			Expr::Binary { op, left, right } => {
				self.emit_int_expr(left)?;
				self.push("    push rax");
				self.emit_int_expr(right)?;
				self.push("    pop rbx");
				match op {
					BinaryOp::Add => {
						self.push("    add rbx, rax");
						self.push("    mov rax, rbx");
					}
					BinaryOp::Sub => {
						self.push("    sub rbx, rax");
						self.push("    mov rax, rbx");
					}
					BinaryOp::Mul => {
						self.push("    imul rbx, rax");
						self.push("    mov rax, rbx");
					}
					BinaryOp::Div => {
						self.push("    mov rcx, rax");
						self.push("    mov rax, rbx");
						self.push("    cqo");
						self.push("    idiv rcx");
					}
				}
			}
			_ => {
				return Err(CompilerError::Codegen(
					"Expected integer expression in integer context".to_string(),
				));
			}
		}
		Ok(())
	}

	fn emit_str_expr(&mut self, expr: &Expr) -> CompilerResult<()> {
		match expr {
			Expr::StrLit(value) => {
				let literal_index = self.string_literal_index(value)?;
				self.push(&format!("    lea rax, [rel str_lit_{literal_index}]"));
				self.push(&format!("    mov rdx, {}", value.len()));
			}
			Expr::StrVar(var) => {
				self.push(&format!("    mov rax, [str_ptr_{}]", str_var_name(*var)));
				self.push(&format!("    mov rdx, [str_len_{}]", str_var_name(*var)));
			}
			Expr::Binary { op, left, right } => {
				if *op != BinaryOp::Add {
					return Err(CompilerError::Codegen(
						"Only '+' is valid for string expressions".to_string(),
					));
				}
				self.emit_str_expr(left)?;
				self.push("    push rax");
				self.push("    push rdx");
				self.emit_str_expr(right)?;
				self.push("    mov r10, rax");
				self.push("    mov r11, rdx");
				self.push("    pop rcx");
				self.push("    pop rbx");
				self.push("    mov r12, rbx");
				self.push("    mov r13, rcx");
				self.push("    mov r14, r10");
				self.push("    mov r15, r11");
				self.push("    mov rdi, r13");
				self.push("    add rdi, r15");
				self.push("    call alloc_str");
				self.push("    mov rbx, rax");
				self.push("    mov rdi, rbx");
				self.push("    mov rsi, r12");
				self.push("    mov rdx, r13");
				self.push("    call memcpy");
				self.push("    lea rdi, [rbx + r13]");
				self.push("    mov rsi, r14");
				self.push("    mov rdx, r15");
				self.push("    call memcpy");
				self.push("    mov rax, rbx");
				self.push("    mov rdx, r13");
				self.push("    add rdx, r15");
			}
			_ => {
				return Err(CompilerError::Codegen(
					"Expected string expression in string context".to_string(),
				));
			}
		}

		Ok(())
	}

	fn emit_fallthrough(&mut self, next: Option<u32>) {
		if let Some(next_line) = next {
			self.push(&format!("    jmp {}", line_label(next_line)));
		} else {
			self.push("    mov rax, 60");
			self.push("    xor rdi, rdi");
			self.push("    syscall");
		}
	}

	fn emit_runtime(&mut self) {
		self.push("print_space:");
		self.push("    mov rax, 1");
		self.push("    mov rdi, 1");
		self.push("    lea rsi, [rel char_space]");
		self.push("    mov rdx, 1");
		self.push("    syscall");
		self.push("    ret");

		self.push("print_newline:");
		self.push("    mov rax, 1");
		self.push("    mov rdi, 1");
		self.push("    lea rsi, [rel char_newline]");
		self.push("    mov rdx, 1");
		self.push("    syscall");
		self.push("    ret");

		self.push("print_str:");
		self.push("    mov rsi, rax");
		self.push("    mov rax, 1");
		self.push("    mov rdi, 1");
		self.push("    syscall");
		self.push("    ret");

		self.push("print_i64:");
		self.push("    push rbx");
		self.push("    push rcx");
		self.push("    push rdx");
		self.push("    push rsi");
		self.push("    push rdi");
		self.push("    push r8");
		self.push("    mov rbx, -9223372036854775808");
		self.push("    cmp rax, rbx");
		self.push("    jne .print_i64_not_min");
		self.push("    mov rax, 1");
		self.push("    mov rdi, 1");
		self.push("    lea rsi, [rel int_min_literal]");
		self.push("    mov rdx, 20");
		self.push("    syscall");
		self.push("    jmp .print_i64_done");
		self.push(".print_i64_not_min:");
		self.push("    lea rsi, [rel int_buf_end]");
		self.push("    xor ecx, ecx");
		self.push("    mov rbx, 10");
		self.push("    cmp rax, 0");
		self.push("    jne .print_i64_nonzero");
		self.push("    dec rsi");
		self.push("    mov byte [rsi], '0'");
		self.push("    inc ecx");
		self.push("    jmp .print_i64_emit");
		self.push(".print_i64_nonzero:");
		self.push("    xor r8d, r8d");
		self.push("    cmp rax, 0");
		self.push("    jge .print_i64_digits");
		self.push("    neg rax");
		self.push("    mov r8b, 1");
		self.push(".print_i64_digits:");
		self.push("    xor rdx, rdx");
		self.push("    div rbx");
		self.push("    add dl, '0'");
		self.push("    dec rsi");
		self.push("    mov [rsi], dl");
		self.push("    inc ecx");
		self.push("    test rax, rax");
		self.push("    jnz .print_i64_digits");
		self.push("    cmp r8b, 0");
		self.push("    je .print_i64_emit");
		self.push("    dec rsi");
		self.push("    mov byte [rsi], '-'");
		self.push("    inc ecx");
		self.push(".print_i64_emit:");
		self.push("    mov rax, 1");
		self.push("    mov rdi, 1");
		self.push("    mov rdx, rcx");
		self.push("    syscall");
		self.push(".print_i64_done:");
		self.push("    pop r8");
		self.push("    pop rdi");
		self.push("    pop rsi");
		self.push("    pop rdx");
		self.push("    pop rcx");
		self.push("    pop rbx");
		self.push("    ret");

		self.push("memcpy:");
		self.push("    test rdx, rdx");
		self.push("    jz .memcpy_done");
		self.push(".memcpy_loop:");
		self.push("    mov al, [rsi]");
		self.push("    mov [rdi], al");
		self.push("    inc rsi");
		self.push("    inc rdi");
		self.push("    dec rdx");
		self.push("    jnz .memcpy_loop");
		self.push(".memcpy_done:");
		self.push("    ret");

		self.push("alloc_str:");
		self.push("    mov rax, [str_heap_ptr]");
		self.push("    mov rcx, rax");
		self.push("    add rcx, rdi");
		self.push("    lea rdx, [rel str_heap_end]");
		self.push("    cmp rcx, rdx");
		self.push("    jbe .alloc_ok");
		self.push("    mov rax, 60");
		self.push("    mov rdi, 2");
		self.push("    syscall");
		self.push(".alloc_ok:");
		self.push("    mov [str_heap_ptr], rcx");
		self.push("    ret");

		self.push("read_line:");
		self.push("    mov rax, 0");
		self.push("    mov rdi, 0");
		self.push("    lea rsi, [rel input_buf]");
		self.push(&format!("    mov rdx, {INPUT_BUFFER_SIZE}"));
		self.push("    syscall");
		self.push("    cmp rax, 0");
		self.push("    jg .read_line_scan");
		self.push("    xor rax, rax");
		self.push("    ret");
		self.push(".read_line_scan:");
		self.push("    mov rcx, 0");
		self.push("    lea r8, [rel input_buf]");
		self.push(".read_line_loop:");
		self.push("    cmp rcx, rax");
		self.push("    jge .read_line_done");
		self.push("    mov bl, [r8 + rcx]");
		self.push("    cmp bl, 10");
		self.push("    je .read_line_trim");
		self.push("    cmp bl, 13");
		self.push("    je .read_line_trim");
		self.push("    inc rcx");
		self.push("    jmp .read_line_loop");
		self.push(".read_line_trim:");
		self.push("    mov rax, rcx");
		self.push("    ret");
		self.push(".read_line_done:");
		self.push("    ret");

		self.push("parse_i64:");
		self.push("    xor rax, rax");
		self.push("    xor rcx, rcx");
		self.push("    mov r8, 1");
		self.push("    xor r9d, r9d");
		self.push(".parse_skip_ws:");
		self.push("    cmp rcx, rsi");
		self.push("    jge .parse_done");
		self.push("    mov bl, [rdi + rcx]");
		self.push("    cmp bl, ' '");
		self.push("    je .parse_skip_advance");
		self.push("    cmp bl, 9");
		self.push("    je .parse_skip_advance");
		self.push("    jmp .parse_sign");
		self.push(".parse_skip_advance:");
		self.push("    inc rcx");
		self.push("    jmp .parse_skip_ws");
		self.push(".parse_sign:");
		self.push("    cmp bl, '-'");
		self.push("    jne .parse_plus");
		self.push("    mov r8, -1");
		self.push("    inc rcx");
		self.push("    jmp .parse_digits");
		self.push(".parse_plus:");
		self.push("    cmp bl, '+'");
		self.push("    jne .parse_digits");
		self.push("    inc rcx");
		self.push(".parse_digits:");
		self.push("    cmp rcx, rsi");
		self.push("    jge .parse_digits_done");
		self.push("    mov bl, [rdi + rcx]");
		self.push("    cmp bl, '0'");
		self.push("    jl .parse_digits_done");
		self.push("    cmp bl, '9'");
		self.push("    jg .parse_digits_done");
		self.push("    imul rax, rax, 10");
		self.push("    movzx rdx, bl");
		self.push("    sub rdx, '0'");
		self.push("    add rax, rdx");
		self.push("    mov r9b, 1");
		self.push("    inc rcx");
		self.push("    jmp .parse_digits");
		self.push(".parse_digits_done:");
		self.push("    cmp r9b, 1");
		self.push("    jne .parse_zero");
		self.push("    cmp r8, 1");
		self.push("    je .parse_done");
		self.push("    neg rax");
		self.push("    ret");
		self.push(".parse_zero:");
		self.push("    xor rax, rax");
		self.push(".parse_done:");
		self.push("    ret");

		self.push("input_i64:");
		self.push("    call read_line");
		self.push("    mov rsi, rax");
		self.push("    lea rdi, [rel input_buf]");
		self.push("    call parse_i64");
		self.push("    ret");

		self.push("input_str:");
		self.push("    call read_line");
		self.push("    mov r8, rax");
		self.push("    mov rdi, r8");
		self.push("    call alloc_str");
		self.push("    mov rbx, rax");
		self.push("    mov rdi, rbx");
		self.push("    lea rsi, [rel input_buf]");
		self.push("    mov rdx, r8");
		self.push("    call memcpy");
		self.push("    mov rax, rbx");
		self.push("    mov rdx, r8");
		self.push("    ret");
	}

	fn emit_data(&mut self) {
		self.push("section .data");
		self.push("char_space db ' '");
		self.push("char_newline db 10");
		self.push("int_min_literal db '-9223372036854775808'");

		for index in 0..self.string_literals.len() {
			let literal = self.string_literals[index].clone();
			self.push(&format!("str_lit_{index}:"));
			self.push(&format!("    db {}", bytes_to_db_line(literal.as_bytes())));
		}

		self.push("section .bss");
		for index in 0..26 {
			let variable = Variable(index);
			self.push(&format!("var_{} resq 1", var_name(variable)));
		}
		for index in 0..26 {
			let variable = StrVariable(index);
			self.push(&format!("str_ptr_{} resq 1", str_var_name(variable)));
			self.push(&format!("str_len_{} resq 1", str_var_name(variable)));
		}
		self.push("int_buf resb 32");
		self.push("int_buf_end:");
		self.push(&format!("str_heap resb {STRING_HEAP_SIZE}"));
		self.push("str_heap_end:");
		self.push("str_heap_ptr resq 1");
		self.push(&format!("input_buf resb {INPUT_BUFFER_SIZE}"));
	}

	fn ensure_target_exists(&self, target: u32) -> CompilerResult<()> {
		if self.line_set.contains(&target) {
			Ok(())
		} else {
			Err(CompilerError::Codegen(format!(
				"Invalid branch target line {}",
				target
			)))
		}
	}

	fn string_literal_index(&self, literal: &str) -> CompilerResult<usize> {
		self.string_literals
			.iter()
			.position(|value| value == literal)
			.ok_or_else(|| {
				CompilerError::Codegen(format!(
					"Internal error: missing string literal '{}'",
					literal
				))
			})
	}

	fn push(&mut self, line: &str) {
		self.output.push_str(line);
		self.output.push('\n');
	}
}

fn collect_string_literals(program: &Program) -> Vec<String> {
	let mut values = Vec::new();
	for statement in program.lines.values() {
		collect_literals_from_statement(statement, &mut values);
	}
	values.sort();
	values.dedup();
	values
}

fn collect_literals_from_statement(statement: &Statement, out: &mut Vec<String>) {
	match statement {
		Statement::Let { expr, .. } | Statement::LetStr { expr, .. } => collect_literals(expr, out),
		Statement::Print { items } => {
			for item in items {
				collect_literals(item, out);
			}
		}
		Statement::IfThen { left, right, .. } => {
			collect_literals(left, out);
			collect_literals(right, out);
		}
		Statement::InputInt { .. }
		| Statement::InputStr { .. }
		| Statement::Goto { .. }
		| Statement::End
		| Statement::Rem => {}
	}
}

fn collect_literals(expr: &Expr, out: &mut Vec<String>) {
	match expr {
		Expr::StrLit(value) => out.push(value.clone()),
		Expr::Binary { left, right, .. } => {
			collect_literals(left, out);
			collect_literals(right, out);
		}
		Expr::Int(_) | Expr::Var(_) | Expr::StrVar(_) => {}
	}
}

fn bytes_to_db_line(bytes: &[u8]) -> String {
	if bytes.is_empty() {
		"0".to_string()
	} else {
		bytes
			.iter()
			.map(|value| value.to_string())
			.collect::<Vec<_>>()
			.join(", ")
	}
}

fn line_label(line_number: u32) -> String {
	format!("line_{line_number}")
}

fn var_name(variable: Variable) -> char {
	variable.as_ascii_letter()
}

fn str_var_name(variable: StrVariable) -> char {
	variable.as_ascii_letter()
}

fn comparison_jump(op: ComparisonOp) -> &'static str {
	match op {
		ComparisonOp::Eq => "je",
		ComparisonOp::Ne => "jne",
		ComparisonOp::Lt => "jl",
		ComparisonOp::Gt => "jg",
		ComparisonOp::Le => "jle",
		ComparisonOp::Ge => "jge",
	}
}

#[cfg(test)]
mod tests {
	use crate::codegen::x86_64::generate_assembly;
	use crate::parser::parse_source;
	use crate::semantic::analyze;

	#[test]
	fn emits_labels_and_print_calls() {
		let program = parse_source("10 LET A = 40 + 2\n20 PRINT A\n30 END\n")
			.expect("parse should pass");
		analyze(&program).expect("semantic should pass");
		let asm = generate_assembly(&program).expect("codegen should pass");

		assert!(asm.contains("line_10:"));
		assert!(asm.contains("line_20:"));
		assert!(asm.contains("line_30:"));
		assert!(asm.contains("call print_i64"));
		assert!(asm.contains("call print_newline"));
	}

	#[test]
	fn emits_conditional_jump_for_if_then() {
		let program = parse_source("10 LET A = 1\n20 IF A <= 1 THEN 40\n30 END\n40 END\n")
			.expect("parse should pass");
		analyze(&program).expect("semantic should pass");
		let asm = generate_assembly(&program).expect("codegen should pass");

		assert!(asm.contains("cmp rbx, rax"));
		assert!(asm.contains("jle line_40"));
	}

	#[test]
	fn emits_string_print_and_input_runtime_calls() {
		let program =
			parse_source("10 INPUT A$\n20 PRINT A$ + \"!\"\n30 END\n").expect("parse should pass");
		analyze(&program).expect("semantic should pass");
		let asm = generate_assembly(&program).expect("codegen should pass");

		assert!(asm.contains("call input_str"));
		assert!(asm.contains("call print_str"));
		assert!(asm.contains("call alloc_str"));
	}
}

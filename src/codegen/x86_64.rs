use std::collections::BTreeSet;

use crate::ast::{BinaryOp, ComparisonOp, Expr, Program, Statement, Variable};
use crate::error::{CompilerError, CompilerResult};

pub fn generate_assembly(program: &Program) -> CompilerResult<String> {
	let mut generator = Generator::new(program);
	generator.generate()?;
	Ok(generator.output)
}

struct Generator<'a> {
	program: &'a Program,
	line_numbers: Vec<u32>,
	line_set: BTreeSet<u32>,
	output: String,
}

impl<'a> Generator<'a> {
	fn new(program: &'a Program) -> Self {
		let line_numbers: Vec<u32> = program.lines.keys().copied().collect();
		let line_set = line_numbers.iter().copied().collect();
		Self {
			program,
			line_numbers,
			line_set,
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
					self.emit_expr(&expr)?;
					self.push(&format!("    mov [var_{}], rax", var_name(var)));
					self.emit_fallthrough(next);
				}
				Statement::Print { items } => {
					for (item_index, item) in items.iter().enumerate() {
						self.emit_expr(item)?;
						self.push("    call print_i64");
						if item_index + 1 != items.len() {
							self.push("    call print_space");
						}
					}
					self.push("    call print_newline");
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
					self.emit_expr(&left)?;
					self.push("    push rax");
					self.emit_expr(&right)?;
					self.push("    pop rbx");
					self.push("    cmp rbx, rax");
					self.push(&format!(
						"    {} {}",
						comparison_jump(op),
						line_label(target)
					));
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

	fn emit_expr(&mut self, expr: &Expr) -> CompilerResult<()> {
		match expr {
			Expr::Int(value) => self.push(&format!("    mov rax, {value}")),
			Expr::Var(var) => self.push(&format!("    mov rax, [var_{}]", var_name(*var))),
			Expr::Binary { op, left, right } => {
				self.emit_expr(left)?;
				self.push("    push rax");
				self.emit_expr(right)?;
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

		self.push("print_i64:");
		self.push("    push rbx");
		self.push("    push rcx");
		self.push("    push rdx");
		self.push("    push rsi");
		self.push("    push rdi");
		self.push("    push r8");
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
		self.push("    pop r8");
		self.push("    pop rdi");
		self.push("    pop rsi");
		self.push("    pop rdx");
		self.push("    pop rcx");
		self.push("    pop rbx");
		self.push("    ret");
	}

	fn emit_data(&mut self) {
		self.push("section .data");
		self.push("char_space db ' '");
		self.push("char_newline db 10");

		self.push("section .bss");
		for index in 0..26 {
			let variable = Variable(index);
			self.push(&format!("var_{} resq 1", var_name(variable)));
		}
		self.push("int_buf resb 32");
		self.push("int_buf_end:");
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

	fn push(&mut self, line: &str) {
		self.output.push_str(line);
		self.output.push('\n');
	}
}

fn line_label(line_number: u32) -> String {
	format!("line_{line_number}")
}

fn var_name(variable: Variable) -> char {
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
}

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn compile_assemble_run_examples() {
    if !tool_exists("nasm") || !tool_exists("ld") {
        eprintln!("Skipping e2e tests because nasm/ld are not available on PATH.");
        return;
    }

    let compiler = PathBuf::from(env!("CARGO_BIN_EXE_mbasicr"));
    let fixtures = ["print_arith", "if_goto", "print_multi"];

    for fixture in fixtures {
        run_fixture(&compiler, fixture).unwrap_or_else(|error| {
            panic!("Fixture '{fixture}' failed: {error}");
        });
    }
}

fn run_fixture(compiler: &Path, fixture: &str) -> Result<(), String> {
    let tests_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let bas_path = tests_dir.join(format!("{fixture}.bas"));
    let expected_path = tests_dir.join(format!("{fixture}.out"));
    let expected = fs::read_to_string(&expected_path)
        .map_err(|error| format!("failed reading expected output: {error}"))?;

    let stamp = unique_stamp();
    let scratch_dir = env::temp_dir().join(format!("mbasicr_e2e_{fixture}_{stamp}"));
    fs::create_dir_all(&scratch_dir)
        .map_err(|error| format!("failed creating scratch dir: {error}"))?;

    let exe_path = scratch_dir.join(fixture);
    let asm_path = scratch_dir.join(format!("{fixture}.asm"));
    let obj_path = scratch_dir.join(format!("{fixture}.o"));

    run_command(
        Command::new(compiler)
            .arg(&bas_path)
            .arg("--emit-asm-only")
            .arg("--asm-out")
            .arg(&asm_path),
        "compiler invocation",
    )?;

    run_command(
        Command::new("nasm")
            .arg("-felf64")
            .arg(&asm_path)
            .arg("-o")
            .arg(&obj_path),
        "nasm invocation",
    )?;

    run_command(
        Command::new("ld").arg(&obj_path).arg("-o").arg(&exe_path),
        "ld invocation",
    )?;

    let output = Command::new(&exe_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("failed running produced executable: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "produced executable exited with status {} and stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let actual = String::from_utf8(output.stdout)
        .map_err(|error| format!("produced executable stdout is not UTF-8: {error}"))?;

    if actual != expected {
        return Err(format!(
            "stdout mismatch\nexpected: {:?}\nactual: {:?}",
            expected, actual
        ));
    }

    let _ = fs::remove_file(&obj_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_dir(&scratch_dir);

    Ok(())
}

fn run_command(command: &mut Command, context: &str) -> Result<(), String> {
    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("{context} failed to start: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    Err(format!(
        "{context} failed with status {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn tool_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn unique_stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

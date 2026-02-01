use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir() -> PathBuf {
    let mut dir = env::temp_dir();
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    dir.push(format!("textpp_test_{}_{}", std::process::id(), id));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn run_textpp(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_textpp"))
        .args(args)
        .output()
        .unwrap()
}

fn fixture_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel)
}

fn run_fixture(input_rel: &str, args: &[&str], expected_rel: &str) {
    let input = fixture_path(input_rel);
    let expected = fixture_path(expected_rel);
    let expected_out = fs::read_to_string(expected).unwrap();

    let mut full_args = Vec::new();
    full_args.extend_from_slice(args);
    full_args.push(input.to_str().unwrap());

    let out = run_textpp(&full_args);
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected_out);
}

#[test]
fn include_with_hash_vars_and_dollar_replacement() {
    let dir = temp_dir();
    let input = dir.join("input.md");
    let include = dir.join("inc/part_x.txt");
    write_file(&include, "value $$VAL$$\n");
    write_file(&input, "hello\n#include \"inc/part_##SUF##.txt\"\n");

    let out = run_textpp(&[
        "-DSUF=x",
        "-DVAL=42",
        input.to_str().unwrap(),
    ]);

    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "hello\nvalue 42\n");
}

#[test]
fn missing_include_is_ignored() {
    let dir = temp_dir();
    let input = dir.join("input.md");
    write_file(&input, "before\n#include \"missing.txt\"\nafter\n");

    let out = run_textpp(&[input.to_str().unwrap()]);

    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "before\nafter\n");
}

#[test]
fn undefined_dollar_var_is_empty() {
    let dir = temp_dir();
    let input = dir.join("input.md");
    write_file(&input, "a $$NOPE$$ b\n");

    let out = run_textpp(&[input.to_str().unwrap()]);

    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "a  b\n");
}

#[test]
fn ifdef_fails_when_defined_as_empty() {
    let dir = temp_dir();
    let input = dir.join("input.md");
    write_file(&input, "#ifdef KEY\nyes\n#else\nno\n#endif\n");

    let out = run_textpp(&["-DKEY=", input.to_str().unwrap()]);

    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "no\n");
}

#[test]
fn if_expression_truthiness_and_comparisons() {
    let dir = temp_dir();
    let input = dir.join("input.md");
    write_file(
        &input,
        "#if (VAR || VAR2 == 3 && VAR3 == \"aaa\" || VAR4 != \"bbb\" || !(VAR3 == \"aaa\" || VAR5==\"ccc\"))\nTRUE\n#else\nFALSE\n#endif\n",
    );

    let out = run_textpp(&[
        "-DVAR=",
        "-DVAR2=3",
        "-DVAR3=aaa",
        "-DVAR4=bbb",
        "-DVAR5=ccc",
        input.to_str().unwrap(),
    ]);

    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "TRUE\n");
}

#[test]
fn invalid_expression_fails() {
    let dir = temp_dir();
    let input = dir.join("input.md");
    write_file(&input, "#if (VAR &&)\nX\n#endif\n");

    let out = run_textpp(&["-DVAR=1", input.to_str().unwrap()]);

    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("invalid expression"));
}

#[test]
fn unmatched_else_fails() {
    let dir = temp_dir();
    let input = dir.join("input.md");
    write_file(&input, "#else\nX\n");

    let out = run_textpp(&[input.to_str().unwrap()]);

    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("invalid directive structure"));
}

#[test]
fn unmatched_endif_fails() {
    let dir = temp_dir();
    let input = dir.join("input.md");
    write_file(&input, "#endif\nX\n");

    let out = run_textpp(&[input.to_str().unwrap()]);

    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("invalid directive structure"));
}

#[test]
fn missing_endif_fails() {
    let dir = temp_dir();
    let input = dir.join("input.md");
    write_file(&input, "#if VAR\nX\n");

    let out = run_textpp(&["-DVAR=1", input.to_str().unwrap()]);

    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("missing #endif"));
}

#[test]
fn directives_require_hash_at_column_zero() {
    let dir = temp_dir();
    let input = dir.join("input.md");
    write_file(&input, " #include \"no.txt\"\nval $$VAL$$\n");

    let out = run_textpp(&["-DVAL=1", input.to_str().unwrap()]);

    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        " #include \"no.txt\"\nval 1\n"
    );
}

#[test]
fn unknown_directives_are_ignored_and_preserved() {
    let dir = temp_dir();
    let input = dir.join("input.md");
    write_file(&input, "#notadirective $$VAL$$\n");

    let out = run_textpp(&["-DVAL=7", input.to_str().unwrap()]);

    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "#notadirective 7\n"
    );
}

#[test]
fn nested_conditions() {
    let dir = temp_dir();
    let input = dir.join("input.md");
    write_file(
        &input,
        "#ifdef OUTER\n#if INNER == \"yes\"\nOK\n#else\nNO\n#endif\n#endif\n",
    );

    let out = run_textpp(&["-DOUTER=1", "-DINNER=yes", input.to_str().unwrap()]);

    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "OK\n");
}

#[test]
fn fixture_basic() {
    run_fixture("valid/basic.md", &["-DNAME=Alice"], "valid/basic.out");
}

#[test]
fn fixture_include_hash() {
    run_fixture(
        "valid/include_hash.md",
        &["-DSUF=x", "-DVAL=99"],
        "valid/include_hash.out",
    );
}

#[test]
fn fixture_expr() {
    run_fixture(
        "valid/expr.md",
        &[
            "-DVAR=",
            "-DVAR2=3",
            "-DVAR3=aaa",
            "-DVAR4=bbb",
            "-DVAR5=ccc",
        ],
        "valid/expr.out",
    );
}

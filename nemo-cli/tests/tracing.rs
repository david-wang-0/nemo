#![cfg(not(miri))]
//! Tests for fact-based tracing through the command line interface.

use std::collections::BTreeSet;

use assert_cmd::cargo_bin_cmd;
use assert_fs::{TempDir, prelude::*};
use serde_json::Value;
use test_log::test;

/// Facts are given inline, so that they are part of the program.
const PROGRAM: &str = "e(1, 2) .
e(2, 3) .
e(3, 4) .
p(?X, ?Y) :- e(?X, ?Y) .
q(?X, ?Z) :- p(?X, ?Y), e(?Y, ?Z) .
r(?X, a), r(?X, b) :- q(?X, ?Z) .
";

/// Run `nmo` with the given tracing arguments on `program`
/// and return the trace it writes.
fn run_trace(program: &str, arguments: &[&str]) -> Value {
    let dir = TempDir::new().expect("temporary directory can be created");
    dir.child("program.rls")
        .write_str(program)
        .expect("program can be written");

    let mut cmd = cargo_bin_cmd!("nmo");
    cmd.current_dir(dir.path())
        .args(["-e", "none", "--trace-output", "trace.json"])
        .args(arguments)
        .arg("program.rls");
    cmd.assert().success();

    let trace =
        std::fs::read_to_string(dir.child("trace.json").path()).expect("trace has been written");
    serde_json::from_str(&trace).expect("trace is valid json")
}

/// Return the final conclusions of a trace, sorted.
fn conclusions(trace: &Value) -> Vec<String> {
    let mut conclusions = serde_json::from_value::<Vec<String>>(trace["finalConclusion"].clone())
        .expect("final conclusions are a list of strings");
    conclusions.sort();
    conclusions
}

/// Return the inferences of a trace as (conclusion, premises) pairs.
fn inferences(trace: &Value) -> BTreeSet<(String, Vec<String>)> {
    trace["inferences"]
        .as_array()
        .expect("inferences are a list")
        .iter()
        .map(|inference| {
            let conclusion = serde_json::from_value(inference["conclusion"].clone())
                .expect("conclusion is a string");
            let premises = serde_json::from_value(inference["premises"].clone())
                .expect("premises are a list of strings");
            (conclusion, premises)
        })
        .collect()
}

fn inference(conclusion: &str, premises: &[&str]) -> (String, Vec<String>) {
    (
        conclusion.to_string(),
        premises.iter().map(|premise| premise.to_string()).collect(),
    )
}

#[test]
fn trace_fact_follows_derivation() {
    let trace = run_trace(PROGRAM, &["--trace", "q(1, 3)"]);

    assert_eq!(conclusions(&trace), vec!["q(1, 3)"]);
    assert_eq!(
        inferences(&trace),
        BTreeSet::from([
            inference("q(1, 3)", &["p(1, 2)", "e(2, 3)"]),
            inference("p(1, 2)", &["e(1, 2)"]),
            inference("e(1, 2)", &[]),
            inference("e(2, 3)", &[]),
        ])
    );
}

#[test]
fn trace_fact_not_derived() {
    let trace = run_trace(PROGRAM, &["--trace", "q(1, 4)"]);
    assert!(conclusions(&trace).is_empty());

    // Unknown predicate
    let trace = run_trace(PROGRAM, &["--trace", "s(1)"]);
    assert!(conclusions(&trace).is_empty());
}

#[test]
fn trace_fact_selects_head_atom() {
    let trace = run_trace(PROGRAM, &["--trace", "r(1, a);r(1, b)"]);

    assert_eq!(conclusions(&trace), vec!["r(1, a)", "r(1, b)"]);
    let inferences = inferences(&trace);
    assert!(inferences.contains(&inference("r(1, a)", &["q(1, 3)"])));
    assert!(inferences.contains(&inference("r(1, b)", &["q(1, 3)"])));
}

#[test]
fn trace_input_file() {
    let dir = TempDir::new().expect("temporary directory can be created");
    dir.child("facts.txt")
        .write_str("q(1, 3);q(2, 4)")
        .expect("facts can be written");
    let facts = dir.child("facts.txt");

    let trace = run_trace(
        PROGRAM,
        &[
            "--trace-input-file",
            facts.path().to_str().expect("path is utf-8"),
        ],
    );

    assert_eq!(conclusions(&trace), vec!["q(1, 3)", "q(2, 4)"]);
}

#[test]
fn trace_all_idb_facts() {
    let trace = run_trace(PROGRAM, &["--trace-all-idb-facts"]);

    assert_eq!(
        conclusions(&trace),
        vec![
            "p(1, 2)", "p(2, 3)", "p(3, 4)", "q(1, 3)", "q(2, 4)", "r(1, a)", "r(1, b)", "r(2, a)",
            "r(2, b)",
        ]
    );
    let inferences = inferences(&trace);
    assert!(inferences.contains(&inference("q(2, 4)", &["p(2, 3)", "e(3, 4)"])));
    assert!(inferences.contains(&inference("r(2, b)", &["q(2, 4)"])));
}

/// Tracing many facts of a program with many inline facts
/// (tracing used to copy the whole program for every traced fact).
#[test]
fn trace_all_idb_facts_many_inline_facts() {
    const FACTS: usize = 500;

    let mut program = String::new();
    for i in 0..FACTS {
        program.push_str(&format!("e({i}, {}) .\n", i + 1));
    }
    program.push_str("p(?X, ?Y) :- e(?X, ?Y) .\n");
    program.push_str("q(?X) :- p(?X, ?Y), e(?Y, ?Z) .\n");

    let trace = run_trace(&program, &["--trace-all-idb-facts"]);

    assert_eq!(conclusions(&trace).len(), 2 * FACTS - 1);
    assert!(inferences(&trace).contains(&inference("q(0)", &["p(0, 1)", "e(1, 2)"])));
}

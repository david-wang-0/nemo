use assert_cmd::cargo_bin_cmd;
use assert_fs::{TempDir, prelude::*};
#[cfg(not(miri))]
use test_log::test;

/// Return the terms of all entries of a node in a tree query response.
fn entry_terms(node: &serde_json::Value) -> Vec<Vec<String>> {
    node["entries"]
        .as_array()
        .expect("node has entries")
        .iter()
        .map(|entry| {
            entry["terms"]
                .as_array()
                .expect("entry has terms")
                .iter()
                .map(|term| term.as_str().expect("term is a string").to_string())
                .collect()
        })
        .collect()
}

/// The variable bound by the traced fact (`?X`) does not come first
/// in the variable order of the rule, which must not affect the tree.
#[cfg_attr(miri, ignore)]
#[test]
fn trace_tree_bound_variable_late_in_order() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let program = dir.child("program.rls");
    program.write_str("e(1, 2) . e(2, 3) . e(3, 4) .\nq(?X) :- e(?X, ?Y), e(?Y, ?Z) .\n")?;
    let query = dir.child("query.json");
    query.write_str(r#"{"predicate":"q","queries":[{"Query":"1"}],"pagination":null}"#)?;

    let mut cmd = cargo_bin_cmd!("nmo");
    cmd.arg("--report")
        .arg("none")
        .arg("-e")
        .arg("none")
        .arg("--trace-tree")
        .arg(query.path())
        .arg(program.path());
    let output = cmd.assert().success().get_output().stdout.clone();
    let tree: serde_json::Value = serde_json::from_slice(&output)?;

    assert_eq!(entry_terms(&tree), vec![vec!["1"]]);

    let children = tree["next"]["children"]
        .as_array()
        .expect("q(1) is derived by a rule");
    let children_terms = children.iter().map(entry_terms).collect::<Vec<_>>();
    assert_eq!(
        children_terms,
        vec![vec![vec!["1", "2"]], vec![vec!["2", "3"]]]
    );

    dir.close()?;
    Ok(())
}

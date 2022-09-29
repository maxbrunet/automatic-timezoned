#[test]
fn cli_tests() {
    trycmd::TestCases::new()
        .case("README.md")
        .case("tests/cmd/*.md")
        .insert_var("[VERSION]", env!("CARGO_PKG_VERSION"))
        .unwrap();
}

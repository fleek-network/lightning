use std::collections::HashMap;

fn main() {
    // Setup the panic hook
    panic_report::setup!();

    // Add some context to the report
    panic_report::add_context("value_one", "string data");
    panic_report::add_context("value_two", vec!["one", "two", "three"]);

    let mut map = HashMap::new();
    map.insert("nested_one", "string data");
    panic_report::add_context("value_three", map);

    // Call chain
    foo()
}

fn foo() {
    bar()
}

fn bar() {
    baz()
}

fn baz() {
    panic!("at the disco")
}

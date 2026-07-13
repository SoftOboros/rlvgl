use dining_philosophers_interactive::{Machine, Value};
fn row(m: &Machine, var: &str) -> String {
    (1..=5)
        .map(|k| match m.get_var(var) {
            Value::Map(ref mm) => format!("{:?}", mm.get(&k.to_string())),
            _ => "?".into(),
        })
        .collect::<Vec<_>>()
        .join(",")
}
#[test]
fn dbg() {
    let mut m = Machine::new();
    m.start();
    m.step("arrive.1", Value::Undefined);
    eprintln!("arrive.1 -> SEATED [{}]", row(&m, "t_SEATED"));
    m.step("arrive.2", Value::Undefined);
    eprintln!(
        "arrive.2 -> SEATED [{}] PHASE [{}]",
        row(&m, "t_SEATED"),
        row(&m, "t_PHASE")
    );
    m.step("arrive.3", Value::Undefined);
    eprintln!("arrive.3 -> SEATED [{}]", row(&m, "t_SEATED"));
}

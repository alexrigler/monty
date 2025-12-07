use monty::Executor;

/// Test we can reuse exec without borrow checker issues.
#[test]
fn repeat_exec() {
    let ex = Executor::new("1 + 2", "test.py", &[]).unwrap();

    let r = ex.run(vec![]).unwrap();
    let int_value: i64 = r.as_ref().try_into().unwrap();
    assert_eq!(int_value, 3);

    let r = ex.run(vec![]).unwrap();
    let int_value: i64 = r.as_ref().try_into().unwrap();
    assert_eq!(int_value, 3);
}

#[test]
fn test_get_interned_string() {
    let ex = Executor::new("'foobar'", "test.py", &[]).unwrap();

    let r = ex.run(vec![]).unwrap();
    let int_value: String = r.as_ref().try_into().unwrap();
    assert_eq!(int_value, "foobar");

    let r = ex.run(vec![]).unwrap();
    let int_value: String = r.as_ref().try_into().unwrap();
    assert_eq!(int_value, "foobar");
}

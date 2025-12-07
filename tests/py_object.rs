use indexmap::IndexMap;
use monty::PyObject;

/// Tests for `PyObject::is_truthy()` - Python's truth value testing rules.

#[test]
fn is_truthy_none_is_falsy() {
    assert!(!PyObject::None.is_truthy());
}

#[test]
fn is_truthy_ellipsis_is_truthy() {
    assert!(PyObject::Ellipsis.is_truthy());
}

#[test]
fn is_truthy_false_is_falsy() {
    assert!(!PyObject::Bool(false).is_truthy());
}

#[test]
fn is_truthy_true_is_truthy() {
    assert!(PyObject::Bool(true).is_truthy());
}

#[test]
fn is_truthy_zero_int_is_falsy() {
    assert!(!PyObject::Int(0).is_truthy());
}

#[test]
fn is_truthy_nonzero_int_is_truthy() {
    assert!(PyObject::Int(1).is_truthy());
    assert!(PyObject::Int(-1).is_truthy());
    assert!(PyObject::Int(42).is_truthy());
}

#[test]
fn is_truthy_zero_float_is_falsy() {
    assert!(!PyObject::Float(0.0).is_truthy());
}

#[test]
fn is_truthy_nonzero_float_is_truthy() {
    assert!(PyObject::Float(1.0).is_truthy());
    assert!(PyObject::Float(-0.5).is_truthy());
    assert!(PyObject::Float(f64::INFINITY).is_truthy());
}

#[test]
fn is_truthy_empty_string_is_falsy() {
    assert!(!PyObject::String(String::new()).is_truthy());
}

#[test]
fn is_truthy_nonempty_string_is_truthy() {
    assert!(PyObject::String("hello".to_string()).is_truthy());
    assert!(PyObject::String(" ".to_string()).is_truthy());
}

#[test]
fn is_truthy_empty_bytes_is_falsy() {
    assert!(!PyObject::Bytes(vec![]).is_truthy());
}

#[test]
fn is_truthy_nonempty_bytes_is_truthy() {
    assert!(PyObject::Bytes(vec![0]).is_truthy());
    assert!(PyObject::Bytes(vec![1, 2, 3]).is_truthy());
}

#[test]
fn is_truthy_empty_list_is_falsy() {
    assert!(!PyObject::List(vec![]).is_truthy());
}

#[test]
fn is_truthy_nonempty_list_is_truthy() {
    assert!(PyObject::List(vec![PyObject::Int(1)]).is_truthy());
}

#[test]
fn is_truthy_empty_tuple_is_falsy() {
    assert!(!PyObject::Tuple(vec![]).is_truthy());
}

#[test]
fn is_truthy_nonempty_tuple_is_truthy() {
    assert!(PyObject::Tuple(vec![PyObject::Int(1)]).is_truthy());
}

#[test]
fn is_truthy_empty_dict_is_falsy() {
    assert!(!PyObject::Dict(IndexMap::new()).is_truthy());
}

#[test]
fn is_truthy_nonempty_dict_is_truthy() {
    let mut dict = IndexMap::new();
    dict.insert(PyObject::String("key".to_string()), PyObject::Int(1));
    assert!(PyObject::Dict(dict).is_truthy());
}

/// Tests for `PyObject::type_name()` - Python type names.

#[test]
fn type_name() {
    assert_eq!(PyObject::None.type_name(), "NoneType");
    assert_eq!(PyObject::Ellipsis.type_name(), "ellipsis");
    assert_eq!(PyObject::Bool(true).type_name(), "bool");
    assert_eq!(PyObject::Bool(false).type_name(), "bool");
    assert_eq!(PyObject::Int(0).type_name(), "int");
    assert_eq!(PyObject::Int(42).type_name(), "int");
    assert_eq!(PyObject::Float(0.0).type_name(), "float");
    assert_eq!(PyObject::Float(2.5).type_name(), "float");
    assert_eq!(PyObject::String(String::new()).type_name(), "str");
    assert_eq!(PyObject::String("hello".to_string()).type_name(), "str");
    assert_eq!(PyObject::Bytes(vec![]).type_name(), "bytes");
    assert_eq!(PyObject::Bytes(vec![1, 2, 3]).type_name(), "bytes");
    assert_eq!(PyObject::List(vec![]).type_name(), "list");
    assert_eq!(PyObject::Tuple(vec![]).type_name(), "tuple");
    assert_eq!(PyObject::Dict(IndexMap::new()).type_name(), "dict");
}

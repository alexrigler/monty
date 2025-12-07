use std::{
    fmt::{self, Write},
    hash::{Hash, Hasher},
};

use indexmap::IndexMap;

use crate::{
    exceptions::{ExcType, SimpleException},
    expressions::FrameExit,
    heap::{Heap, HeapData},
    value::Value,
    values::{
        bytes::{bytes_repr, Bytes},
        dict::Dict,
        list::List,
        str::{string_repr, Str},
        tuple::Tuple,
        PyTrait,
    },
};

/// A Python value that can be passed to or returned from the interpreter.
///
/// This is the public-facing type for Python values. It owns all its data and can be
/// freely cloned, serialized, or stored. Unlike the internal `Value` type, `PyObject`
/// does not require a heap for operations.
///
/// # Input vs Output Variants
///
/// Most variants can be used both as inputs (passed to `Executor::run()`) and outputs
/// (returned from execution). However:
/// - `Repr` is output-only: represents values that have no direct `PyObject` mapping
/// - `Exception` can be used as input (to raise) or output (when code raises)
///
/// # Hashability
///
/// Only immutable variants (`None`, `Ellipsis`, `Bool`, `Int`, `Float`, `String`, `Bytes`)
/// implement `Hash`. Attempting to hash mutable variants (`List`, `Dict`) will panic.
#[derive(Debug, Clone)]
pub enum PyObject {
    /// Python's `Ellipsis` singleton (`...`).
    Ellipsis,
    /// Python's `None` singleton.
    None,
    /// Python boolean (`True` or `False`).
    Bool(bool),
    /// Python integer (64-bit signed).
    Int(i64),
    /// Python float (64-bit IEEE 754).
    Float(f64),
    /// Python string (UTF-8).
    String(String),
    /// Python bytes object.
    Bytes(Vec<u8>),
    /// Python list (mutable sequence).
    List(Vec<PyObject>),
    /// Python tuple (immutable sequence).
    Tuple(Vec<PyObject>),
    /// Python dictionary (insertion-ordered mapping).
    Dict(IndexMap<PyObject, PyObject>),
    /// Python exception with type and optional message argument.
    Exception {
        /// The exception type (e.g., `ValueError`, `TypeError`).
        exc_type: ExcType,
        /// Optional string argument passed to the exception constructor.
        arg: Option<String>,
    },
    /// Fallback for values that cannot be represented as other variants.
    ///
    /// Contains the `repr()` string of the original value. This is output-only
    /// and cannot be used as an input to `Executor::run()`.
    Repr(String),
}

impl fmt::Display for PyObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(s) => f.write_str(s),
            _ => self.repr_fmt(f),
        }
    }
}

impl PyObject {
    /// Converts a `FrameExit` into a `PyObject`, properly handling reference counting.
    ///
    /// Takes ownership of the `FrameExit` and its contained `Value`, extracts the value,
    /// then properly drops the Value via `drop_with_heap` to maintain reference counting.
    pub(crate) fn new<'c, 'e>(exit: FrameExit<'c, 'e>, heap: &mut Heap<'c, 'e>) -> Self {
        match exit {
            FrameExit::Return(obj) => {
                let value = Self::from_value(&obj, heap);
                obj.drop_with_heap(heap);
                value
            }
        }
    }

    /// Converts this `PyObject` into an `Value`, allocating on the heap if needed.
    ///
    /// Immediate values (None, Bool, Int, Float, Ellipsis, Exception) are created directly.
    /// Heap-allocated values (String, Bytes, List, Tuple, Dict) are allocated
    /// via the heap and wrapped in `Value::Ref`.
    ///
    /// # Errors
    /// Returns `InvalidInputError` if called on the `Repr` variant,
    /// as it is only valid as an output from code execution, not as an input.
    pub(crate) fn to_value<'c, 'e>(self, heap: &mut Heap<'c, 'e>) -> Result<Value<'c, 'e>, InvalidInputError> {
        match self {
            Self::Ellipsis => Ok(Value::Ellipsis),
            Self::None => Ok(Value::None),
            Self::Bool(b) => Ok(Value::Bool(b)),
            Self::Int(i) => Ok(Value::Int(i)),
            Self::Float(f) => Ok(Value::Float(f)),
            Self::String(s) => Ok(Value::Ref(heap.allocate(HeapData::Str(Str::new(s))))),
            Self::Bytes(b) => Ok(Value::Ref(heap.allocate(HeapData::Bytes(Bytes::new(b))))),
            Self::List(items) => {
                let values: Vec<Value<'c, 'e>> = items
                    .into_iter()
                    .map(|item| item.to_value(heap))
                    .collect::<Result<_, _>>()?;
                Ok(Value::Ref(heap.allocate(HeapData::List(List::new(values)))))
            }
            Self::Tuple(items) => {
                let values: Vec<Value<'c, 'e>> = items
                    .into_iter()
                    .map(|item| item.to_value(heap))
                    .collect::<Result<_, _>>()?;
                Ok(Value::Ref(heap.allocate(HeapData::Tuple(Tuple::from_vec(values)))))
            }
            Self::Dict(map) => {
                let pairs: Result<Vec<(Value<'c, 'e>, Value<'c, 'e>)>, InvalidInputError> = map
                    .into_iter()
                    .map(|(k, v)| Ok((k.to_value(heap)?, v.to_value(heap)?)))
                    .collect();
                // PyObject Dict keys are already validated as hashable, so unwrap is safe
                let dict = Dict::from_pairs(pairs?, heap).expect("PyObject Dict should only contain hashable keys");
                Ok(Value::Ref(heap.allocate(HeapData::Dict(dict))))
            }
            Self::Exception { exc_type, arg } => {
                let exc = SimpleException::new(exc_type, arg.map(Into::into));
                Ok(Value::Exc(exc))
            }
            Self::Repr(_) => Err(InvalidInputError::new("Repr")),
        }
    }

    fn from_value(object: &Value, heap: &Heap) -> Self {
        match object {
            Value::Undefined => panic!("Undefined found while converting to PyObject"),
            Value::Ellipsis => Self::Ellipsis,
            Value::None => Self::None,
            Value::Bool(b) => Self::Bool(*b),
            Value::Int(i) => Self::Int(*i),
            Value::Float(f) => Self::Float(*f),
            Value::InternString(s) => Self::String((*s).to_owned()),
            Value::InternBytes(b) => Self::Bytes((*b).to_owned()),
            Value::Exc(exc) => Self::Exception {
                exc_type: exc.exc_type(),
                arg: exc.arg().map(ToString::to_string),
            },
            Value::Ref(id) => match heap.get(*id) {
                HeapData::Str(s) => Self::String(s.as_str().to_owned()),
                HeapData::Bytes(b) => Self::Bytes(b.as_slice().to_owned()),
                HeapData::List(list) => Self::List(
                    list.as_vec()
                        .iter()
                        .map(|obj| PyObject::from_value(obj, heap))
                        .collect(),
                ),
                HeapData::Tuple(tuple) => Self::Tuple(
                    tuple
                        .as_vec()
                        .iter()
                        .map(|obj| PyObject::from_value(obj, heap))
                        .collect(),
                ),
                HeapData::Dict(dict) => {
                    let mut new_dict = IndexMap::with_capacity(dict.as_index_map().len());
                    for bucket in dict.as_index_map().values() {
                        for (k, v) in bucket {
                            new_dict.insert(PyObject::from_value(k, heap), PyObject::from_value(v, heap));
                        }
                    }
                    Self::Dict(new_dict)
                }
            },
            #[cfg(feature = "dec-ref-check")]
            Value::Dereferenced => panic!("Dereferenced found while converting to PyObject"),
            _ => Self::Repr(object.py_repr(heap).into_owned()),
        }
    }

    /// Returns the Python `repr()` string for this value.
    ///
    /// # Panics
    /// Could panic if out of memory.
    #[must_use]
    pub fn py_repr(&self) -> String {
        let mut s = String::new();
        self.repr_fmt(&mut s).expect("Unable to format repr display value");
        s
    }

    fn repr_fmt(&self, f: &mut impl Write) -> fmt::Result {
        match self {
            Self::Ellipsis => f.write_str("Ellipsis"),
            Self::None => f.write_str("None"),
            Self::Bool(true) => f.write_str("True"),
            Self::Bool(false) => f.write_str("False"),
            Self::Int(v) => write!(f, "{v}"),
            Self::Float(v) => {
                let s = v.to_string();
                f.write_str(&s)?;
                if !s.contains('.') {
                    f.write_str(".0")?;
                }
                Ok(())
            }
            Self::String(s) => f.write_str(&string_repr(s)),
            Self::Bytes(b) => f.write_str(&bytes_repr(b)),
            Self::List(l) => {
                f.write_char('[')?;
                let mut iter = l.iter();
                if let Some(first) = iter.next() {
                    first.repr_fmt(f)?;
                    for item in iter {
                        f.write_str(", ")?;
                        item.repr_fmt(f)?;
                    }
                }
                f.write_char(']')
            }
            Self::Tuple(t) => {
                f.write_char('(')?;
                let mut iter = t.iter();
                if let Some(first) = iter.next() {
                    first.repr_fmt(f)?;
                    for item in iter {
                        f.write_str(", ")?;
                        item.repr_fmt(f)?;
                    }
                }
                f.write_char(')')
            }
            Self::Dict(d) => {
                f.write_char('{')?;
                let mut iter = d.iter();
                if let Some((k, v)) = iter.next() {
                    k.repr_fmt(f)?;
                    f.write_str(": ")?;
                    v.repr_fmt(f)?;
                    for (k, v) in iter {
                        f.write_str(", ")?;
                        k.repr_fmt(f)?;
                        f.write_str(": ")?;
                        v.repr_fmt(f)?;
                    }
                }
                f.write_char('}')
            }
            Self::Exception { exc_type, arg } => {
                let type_str: &'static str = exc_type.into();
                write!(f, "{type_str}(")?;

                if let Some(arg) = &arg {
                    f.write_str(&string_repr(arg))?;
                }
                f.write_char(')')
            }
            Self::Repr(s) => write!(f, "Repr({})", string_repr(s)),
        }
    }

    /// Returns `true` if this value is "truthy" according to Python's truth testing rules.
    ///
    /// In Python, the following values are considered falsy:
    /// - `None` and `Ellipsis`
    /// - `False`
    /// - Zero numeric values (`0`, `0.0`)
    /// - Empty sequences and collections (`""`, `b""`, `[]`, `()`, `{}`)
    ///
    /// All other values are truthy, including `Exception` and `Repr` variants.
    #[must_use]
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::None => false,
            Self::Ellipsis => true,
            Self::Bool(b) => *b,
            Self::Int(i) => *i != 0,
            Self::Float(f) => *f != 0.0,
            Self::String(s) => !s.is_empty(),
            Self::Bytes(b) => !b.is_empty(),
            Self::List(l) => !l.is_empty(),
            Self::Tuple(t) => !t.is_empty(),
            Self::Dict(d) => !d.is_empty(),
            Self::Exception { .. } => true,
            Self::Repr(_) => true,
        }
    }

    /// Returns the Python type name for this value (e.g., `"int"`, `"str"`, `"list"`).
    ///
    /// These are the same names returned by Python's `type(x).__name__`.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::None => "NoneType",
            Self::Ellipsis => "ellipsis",
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::String(_) => "str",
            Self::Bytes(_) => "bytes",
            Self::List(_) => "list",
            Self::Tuple(_) => "tuple",
            Self::Dict(_) => "dict",
            Self::Exception { .. } => "Exception",
            Self::Repr(_) => "repr",
        }
    }
}

impl Hash for PyObject {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash the discriminant first
        std::mem::discriminant(self).hash(state);

        match self {
            Self::Ellipsis => {}
            Self::None => {}
            Self::Bool(bool) => bool.hash(state),
            Self::Int(i64) => i64.hash(state),
            Self::Float(f64) => f64.to_bits().hash(state),
            Self::String(string) => string.hash(state),
            Self::Bytes(bytes) => bytes.hash(state),
            _ => panic!("{} python values are not hashable", self.type_name()),
        }
    }
}

impl PartialEq for PyObject {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Ellipsis, Self::Ellipsis) => true,
            (Self::None, Self::None) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            // Use to_bits() for float comparison to be consistent with Hash
            (Self::Float(a), Self::Float(b)) => a.to_bits() == b.to_bits(),
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Bytes(a), Self::Bytes(b)) => a == b,
            (Self::List(a), Self::List(b)) => a == b,
            (Self::Tuple(a), Self::Tuple(b)) => a == b,
            (Self::Dict(a), Self::Dict(b)) => a == b,
            (
                Self::Exception {
                    exc_type: a_type,
                    arg: a_arg,
                },
                Self::Exception {
                    exc_type: b_type,
                    arg: b_arg,
                },
            ) => a_type == b_type && a_arg == b_arg,
            (Self::Repr(a), Self::Repr(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for PyObject {}

impl AsRef<PyObject> for PyObject {
    fn as_ref(&self) -> &PyObject {
        self
    }
}

/// Error returned when a `PyObject` cannot be converted to the requested Rust type.
///
/// This error is returned by the `TryFrom` implementations when attempting to extract
/// a specific type from a `PyObject` that holds a different variant.
///
/// # Example
///
/// ```ignore
/// let obj = PyObject::String("hello".to_string());
/// let result: Result<i64, ConversionError> = (&obj).try_into();
/// assert!(result.is_err());
/// ```
#[derive(Debug)]
pub struct ConversionError {
    /// The type name that was expected (e.g., "int", "str").
    pub expected: &'static str,
    /// The actual type name of the `PyObject` (e.g., "list", "NoneType").
    pub actual: &'static str,
}

impl ConversionError {
    /// Creates a new `ConversionError` with the expected and actual type names.
    #[must_use]
    pub fn new(expected: &'static str, actual: &'static str) -> Self {
        Self { expected, actual }
    }
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expected {}, got {}", self.expected, self.actual)
    }
}

impl std::error::Error for ConversionError {}

/// Error returned when a `PyObject` cannot be used as an input to code execution.
///
/// Some `PyObject` variants (like `Repr`) are only valid as outputs
/// from code execution, not as inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidInputError {
    /// The type name of the invalid input value
    pub type_name: &'static str,
}

impl InvalidInputError {
    /// Creates a new `InvalidInputError` for the given type name.
    #[must_use]
    pub fn new(type_name: &'static str) -> Self {
        Self { type_name }
    }
}

impl fmt::Display for InvalidInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "'{}' is not a valid input value", self.type_name)
    }
}

impl std::error::Error for InvalidInputError {}

/// Attempts to convert a PyObject to an i64 integer.
/// Returns an error if the object is not an Int variant.
impl TryFrom<&PyObject> for i64 {
    type Error = ConversionError;

    fn try_from(value: &PyObject) -> Result<Self, Self::Error> {
        match value {
            PyObject::Int(i) => Ok(*i),
            _ => Err(ConversionError::new("int", value.type_name())),
        }
    }
}

/// Attempts to convert a PyObject to an f64 float.
/// Returns an error if the object is not a Float or Int variant.
/// Int values are automatically converted to f64 to match python's behavior.
impl TryFrom<&PyObject> for f64 {
    type Error = ConversionError;

    fn try_from(value: &PyObject) -> Result<Self, Self::Error> {
        match value {
            PyObject::Float(f) => Ok(*f),
            PyObject::Int(i) => Ok(*i as f64),
            _ => Err(ConversionError::new("float", value.type_name())),
        }
    }
}

/// Attempts to convert a PyObject to a String.
/// Returns an error if the object is not a heap-allocated Str variant.
impl TryFrom<&PyObject> for String {
    type Error = ConversionError;

    fn try_from(value: &PyObject) -> Result<Self, Self::Error> {
        if let PyObject::String(s) = value {
            Ok(s.clone())
        } else {
            Err(ConversionError::new("str", value.type_name()))
        }
    }
}

/// Attempts to convert a `PyObject` to a bool.
/// Returns an error if the object is not a True or False variant.
/// Note: This does NOT use Python's truthiness rules (use PyObject::bool for that).
impl TryFrom<&PyObject> for bool {
    type Error = ConversionError;

    fn try_from(value: &PyObject) -> Result<Self, Self::Error> {
        match value {
            PyObject::Bool(b) => Ok(*b),
            _ => Err(ConversionError::new("bool", value.type_name())),
        }
    }
}

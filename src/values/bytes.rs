/// Python bytes type, wrapping a `Vec<u8>`.
///
/// This type provides Python bytes semantics. Currently supports basic
/// operations like length and equality comparison.
use std::borrow::Cow;
use std::fmt::Write;

use crate::args::ArgValues;
use crate::exceptions::ExcType;
use crate::heap::{Heap, HeapId};
use crate::run::RunResult;
use crate::value::{Attr, Value};
use crate::values::PyTrait;

/// Python bytes value stored on the heap.
///
/// Wraps a `Vec<u8>` and provides Python-compatible operations.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Bytes(Vec<u8>);

impl Bytes {
    /// Creates a new Bytes from a byte vector.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Returns a reference to the inner byte slice.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// Returns a mutable reference to the inner byte vector.
    pub fn as_vec_mut(&mut self) -> &mut Vec<u8> {
        &mut self.0
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl From<&[u8]> for Bytes {
    fn from(bytes: &[u8]) -> Self {
        Self(bytes.to_vec())
    }
}

impl From<Bytes> for Vec<u8> {
    fn from(bytes: Bytes) -> Self {
        bytes.0
    }
}

impl std::ops::Deref for Bytes {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'c, 'e> PyTrait<'c, 'e> for Bytes {
    fn py_type(&self, _heap: &Heap<'c, 'e>) -> &'static str {
        "bytes"
    }

    fn py_len(&self, _heap: &Heap<'c, 'e>) -> Option<usize> {
        Some(self.0.len())
    }

    fn py_eq(&self, other: &Self, _heap: &mut Heap<'c, 'e>) -> bool {
        self.0 == other.0
    }

    /// Bytes don't contain nested heap references.
    fn py_dec_ref_ids(&mut self, _stack: &mut Vec<HeapId>) {
        // No-op: bytes don't hold Value references
    }

    fn py_bool(&self, _heap: &Heap<'c, 'e>) -> bool {
        !self.0.is_empty()
    }

    fn py_repr<'a>(&'a self, _heap: &'a Heap<'c, 'e>) -> Cow<'a, str> {
        Cow::Owned(bytes_repr(&self.0))
    }

    fn py_call_attr(
        &mut self,
        heap: &mut Heap<'c, 'e>,
        attr: &Attr,
        _args: ArgValues<'c, 'e>,
    ) -> RunResult<'static, Value<'c, 'e>> {
        Err(ExcType::attribute_error(self.py_type(heap), attr))
    }
}

/// Returns a CPython-compatible repr string for bytes.
///
/// Format: `b'...'` or `b"..."` depending on content.
/// - Uses single quotes by default
/// - Switches to double quotes if bytes contain `'` but not `"`
/// - Escapes: `\\`, `\t`, `\n`, `\r`, `\xNN` for non-printable bytes
#[must_use]
pub fn bytes_repr(bytes: &[u8]) -> String {
    // Determine quote character: use double quotes if single quote present but not double
    let has_single = bytes.contains(&b'\'');
    let has_double = bytes.contains(&b'"');
    let quote = if has_single && !has_double { '"' } else { '\'' };

    let mut result = String::with_capacity(bytes.len() + 3);
    result.push('b');
    result.push(quote);

    for &byte in bytes {
        match byte {
            b'\\' => result.push_str("\\\\"),
            b'\t' => result.push_str("\\t"),
            b'\n' => result.push_str("\\n"),
            b'\r' => result.push_str("\\r"),
            b'\'' if quote == '\'' => result.push_str("\\'"),
            b'"' if quote == '"' => result.push_str("\\\""),
            // Printable ASCII (32-126)
            0x20..=0x7e => result.push(byte as char),
            // Non-printable: use \xNN format
            _ => {
                let _ = write!(result, "\\x{byte:02x}");
            }
        }
    }

    result.push(quote);
    result
}

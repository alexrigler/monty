/// Built-in functions for the Python interpreter.
///
/// This module contains the `Builtins` enum representing all supported built-in
/// functions (print, len, str, etc.).
use strum::{AsRefStr, Display, EnumString};

use crate::args::ArgValues;
use crate::exceptions::{exc_err_fmt, ExcType};
use crate::heap::{Heap, HeapData};
use crate::run::RunResult;
use crate::value::Value;
use crate::values::PyTrait;

/// Enumerates every interpreter-native Python builtin Monty currently supports.
///
/// Uses strum derives for automatic `Display`, `FromStr`, and `AsRef<str>` implementations.
/// All variants serialize to lowercase (e.g., `Print` -> "print").
#[derive(Debug, Clone, Copy, Display, EnumString, AsRefStr)]
#[strum(serialize_all = "lowercase")]
pub enum Builtins {
    Print,
    Len,
    Str,
    Repr,
    Id,
    Range,
    Hash,
}

impl Builtins {
    /// Executes the builtin with the provided positional arguments.
    pub fn call<'c, 'e>(self, heap: &mut Heap<'c, 'e>, args: ArgValues<'c, 'e>) -> RunResult<'c, Value<'c, 'e>> {
        match self {
            Self::Print => {
                match args {
                    ArgValues::Zero => {}
                    ArgValues::One(a) => {
                        println!("{}", a.py_str(heap));
                        a.drop_with_heap(heap);
                    }
                    ArgValues::Two(a1, a2) => {
                        println!("{} {}", a1.py_str(heap), a2.py_str(heap));
                        a1.drop_with_heap(heap);
                        a2.drop_with_heap(heap);
                    }
                    ArgValues::Many(args) => {
                        let mut iter = args.iter();
                        print!("{}", iter.next().unwrap().py_str(heap));
                        for value in iter {
                            print!(" {}", value.py_str(heap));
                        }
                        println!();
                        // Clean up all args
                        for arg in args {
                            arg.drop_with_heap(heap);
                        }
                    }
                }
                Ok(Value::None)
            }
            Self::Len => {
                let value = args.get_one_arg("len")?;
                let result = match value.py_len(heap) {
                    Some(len) => Ok(Value::Int(len as i64)),
                    None => exc_err_fmt!(ExcType::TypeError; "object of type {} has no len()", value.py_repr(heap)),
                };
                value.drop_with_heap(heap);
                result
            }
            Self::Str => {
                let value = args.get_one_arg("str")?;
                let heap_id = heap.allocate(HeapData::Str(value.py_str(heap).into_owned().into()));
                value.drop_with_heap(heap);
                Ok(Value::Ref(heap_id))
            }
            Self::Repr => {
                let value = args.get_one_arg("repr")?;
                let heap_id = heap.allocate(HeapData::Str(value.py_repr(heap).into_owned().into()));
                value.drop_with_heap(heap);
                Ok(Value::Ref(heap_id))
            }
            Self::Id => {
                let value = args.get_one_arg("id")?;
                let id = value.id();
                // For heap values, we intentionally don't drop to prevent heap slot reuse
                // which would cause id([]) == id([]) to return True (same slot reused).
                // For immediate values, dropping is a no-op since they don't use heap slots.
                // This is an acceptable trade-off: small leak for heap values passed to id(),
                // but correct semantics for value identity.
                if matches!(value, Value::Ref(_)) {
                    #[cfg(feature = "dec-ref-check")]
                    std::mem::forget(value);
                } else {
                    value.drop_with_heap(heap);
                }
                Ok(Value::Int(id as i64))
            }
            Self::Range => {
                let value = args.get_one_arg("range")?;
                let result = value.as_int();
                value.drop_with_heap(heap);
                Ok(Value::Range(result?))
            }
            Self::Hash => {
                let value = args.get_one_arg("hash")?;
                let result = match value.py_hash_u64(heap) {
                    Some(hash) => Ok(Value::Int(hash as i64)),
                    None => Err(ExcType::type_error_unhashable(value.py_type(heap))),
                };
                value.drop_with_heap(heap);
                result
            }
        }
    }
}

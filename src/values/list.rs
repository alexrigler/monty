use std::borrow::Cow;

use crate::args::ArgValues;
use crate::exceptions::ExcType;
use crate::heap::{Heap, HeapData, HeapId};
use crate::run::RunResult;
use crate::value::{Attr, Value};
use crate::values::PyTrait;

/// Python list type, wrapping a Vec of Values.
///
/// This type provides Python list semantics including dynamic growth,
/// reference counting for heap values, and standard list methods like
/// append and insert.
///
/// # Reference Counting
/// When values are added to the list (via append, insert, etc.), their
/// reference counts are incremented if they are heap-allocated (Ref variants).
/// This ensures values remain valid while referenced by the list.
#[derive(Debug, Default)]
pub struct List<'c, 'e>(Vec<Value<'c, 'e>>);

impl<'c, 'e> List<'c, 'e> {
    /// Creates a new list from a vector of values.
    ///
    /// Note: This does NOT increment reference counts - the caller must
    /// ensure refcounts are properly managed.
    #[must_use]
    pub fn new(vec: Vec<Value<'c, 'e>>) -> Self {
        Self(vec)
    }

    /// Returns a reference to the underlying vector.
    #[must_use]
    pub fn as_vec(&self) -> &Vec<Value<'c, 'e>> {
        &self.0
    }

    /// Returns a mutable reference to the underlying vector.
    ///
    /// # Safety Considerations
    /// Be careful when mutating the vector directly - you must manually
    /// manage reference counts for any heap values you add or remove.
    pub fn as_vec_mut(&mut self) -> &mut Vec<Value<'c, 'e>> {
        &mut self.0
    }

    /// Returns the number of elements in the list.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Creates a deep clone of this list with proper reference counting.
    ///
    /// All heap-allocated values in the list have their reference counts
    /// incremented. This should be used instead of `.clone()` which would
    /// bypass reference counting.
    #[must_use]
    pub fn clone_with_heap(&self, heap: &mut Heap<'c, 'e>) -> Self {
        let cloned: Vec<Value<'c, 'e>> = self.0.iter().map(|obj| obj.clone_with_heap(heap)).collect();
        Self(cloned)
    }

    /// Appends an element to the end of the list.
    ///
    /// The caller transfers ownership of `item` to the list. The item's refcount
    /// is NOT incremented here - the caller is responsible for ensuring the refcount
    /// was already incremented (e.g., via `clone_with_heap` or `evaluate_use`).
    ///
    /// Returns `Value::None`, matching Python's behavior where `list.append()` returns None.
    pub fn append(&mut self, _heap: &mut Heap<'c, 'e>, item: Value<'c, 'e>) {
        // Ownership transfer - refcount was already handled by caller
        self.0.push(item);
    }

    /// Inserts an element at the specified index.
    ///
    /// The caller transfers ownership of `item` to the list. The item's refcount
    /// is NOT incremented here - the caller is responsible for ensuring the refcount
    /// was already incremented.
    ///
    /// # Arguments
    /// * `index` - The position to insert at (0-based). If index >= len(),
    ///   the item is appended to the end (matching Python semantics).
    ///
    /// Returns `Value::None`, matching Python's behavior where `list.insert()` returns None.
    pub fn insert(&mut self, _heap: &mut Heap<'c, 'e>, index: usize, item: Value<'c, 'e>) {
        // Ownership transfer - refcount was already handled by caller
        // Python's insert() appends if index is out of bounds
        if index >= self.0.len() {
            self.0.push(item);
        } else {
            self.0.insert(index, item);
        }
    }
}

impl<'c, 'e> From<List<'c, 'e>> for Vec<Value<'c, 'e>> {
    fn from(list: List<'c, 'e>) -> Self {
        list.0
    }
}

impl<'c, 'e> PyTrait<'c, 'e> for List<'c, 'e> {
    fn py_type(&self, _heap: &Heap<'c, 'e>) -> &'static str {
        "list"
    }

    fn py_len(&self, _heap: &Heap<'c, 'e>) -> Option<usize> {
        Some(self.0.len())
    }

    fn py_getitem(&self, key: &Value<'c, 'e>, heap: &mut Heap<'c, 'e>) -> RunResult<'c, Value<'c, 'e>> {
        // Extract integer index from key, returning TypeError if not an int
        let index = match key {
            Value::Int(i) => *i,
            _ => return Err(ExcType::type_error_indices("list", key.py_type(heap))),
        };

        // Convert to usize, handling negative indices (Python-style: -1 = last element)
        let len = self.0.len() as i64;
        let normalized_index = if index < 0 { index + len } else { index };

        // Bounds check
        if normalized_index < 0 || normalized_index >= len {
            return Err(ExcType::list_index_error());
        }

        // Return clone of the item with proper refcount increment
        Ok(self.0[normalized_index as usize].clone_with_heap(heap))
    }

    fn py_eq(&self, other: &Self, heap: &mut Heap<'c, 'e>) -> bool {
        if self.0.len() != other.0.len() {
            return false;
        }
        for (i1, i2) in self.0.iter().zip(&other.0) {
            if !i1.py_eq(i2, heap) {
                return false;
            }
        }
        true
    }

    fn py_dec_ref_ids(&mut self, stack: &mut Vec<HeapId>) {
        for obj in &mut self.0 {
            if let Value::Ref(id) = obj {
                stack.push(*id);
                #[cfg(feature = "dec-ref-check")]
                obj.dec_ref_forget();
            }
        }
    }

    fn py_bool(&self, _heap: &Heap<'c, 'e>) -> bool {
        !self.0.is_empty()
    }

    fn py_repr<'a>(&'a self, heap: &'a Heap<'c, 'e>) -> Cow<'a, str> {
        Cow::Owned(repr_sequence('[', ']', &self.0, heap))
    }

    fn py_add(&self, other: &Self, heap: &mut Heap<'c, 'e>) -> Option<Value<'c, 'e>> {
        // Clone both lists' contents with proper refcounting
        let mut result: Vec<Value<'c, 'e>> = self.0.iter().map(|obj| obj.clone_with_heap(heap)).collect();
        let other_cloned: Vec<Value<'c, 'e>> = other.0.iter().map(|obj| obj.clone_with_heap(heap)).collect();
        result.extend(other_cloned);
        let id = heap.allocate(HeapData::List(List::new(result)));
        Some(Value::Ref(id))
    }

    fn py_iadd(&mut self, other: Value<'c, 'e>, heap: &mut Heap<'c, 'e>, self_id: Option<HeapId>) -> bool {
        // Extract the value ID first, keeping `other` around to drop later
        let Value::Ref(other_id) = &other else { return false };

        if Some(*other_id) == self_id {
            // Self-extend: clone our own items with proper refcounting
            let items = self.0.iter().map(|obj| obj.clone_with_heap(heap)).collect::<Vec<_>>();
            self.0.extend(items);
        } else {
            // Get items from other list using iadd_extend_from_heap helper
            // This handles the borrow checker limitations with lifetime propagation
            if !heap.iadd_extend_list(*other_id, &mut self.0) {
                return false;
            }
        }

        // Drop the other value - we've extracted its contents and are done with the temporary reference
        other.drop_with_heap(heap);
        true
    }

    fn py_call_attr(
        &mut self,
        heap: &mut Heap<'c, 'e>,
        attr: &Attr,
        args: ArgValues<'c, 'e>,
    ) -> RunResult<'c, Value<'c, 'e>> {
        match attr {
            Attr::Append => {
                let item = args.get_one_arg("list.append")?;
                self.append(heap, item);
                Ok(Value::None)
            }
            Attr::Insert => {
                let (index_obj, item) = args.get_two_args("insert")?;
                let index = index_obj.as_int()? as usize;
                self.insert(heap, index, item);
                Ok(Value::None)
            }
            _ => Err(ExcType::attribute_error("list", attr)),
        }
    }
}

/// Formats a sequence of values with the given start and end characters.
///
/// This helper function is used to implement `__repr__` for sequence types like
/// lists and tuples. It formats items as comma-separated repr strings.
///
/// # Arguments
/// * `start` - The opening character (e.g., '[' for lists, '(' for tuples)
/// * `end` - The closing character (e.g., ']' for lists, ')' for tuples)
/// * `items` - The slice of values to format
/// * `heap` - The heap for resolving value references
///
/// # Returns
/// A string representation like "[1, 2, 3]" or "(1, 2, 3)"
pub(crate) fn repr_sequence<'c, 'e>(start: char, end: char, items: &[Value<'c, 'e>], heap: &Heap<'c, 'e>) -> String {
    let mut s = String::from(start);
    let mut iter = items.iter();
    if let Some(first) = iter.next() {
        let repr = first.py_repr(heap);
        s.push_str(repr.as_ref());
        for item in iter {
            s.push_str(", ");
            let repr = item.py_repr(heap);
            s.push_str(repr.as_ref());
        }
    }
    s.push(end);
    s
}

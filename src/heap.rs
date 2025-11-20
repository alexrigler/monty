use crate::exceptions::Exception;
use crate::object::Object;

/// Unique identifier for objects stored inside the heap arena.
pub type ObjectId = usize;

/// HeapData captures every runtime object that must live in the arena.
#[derive(Debug)]
pub enum HeapData {
    Str(String),
    Bytes(Vec<u8>),
    List(Vec<Object>),
    Tuple(Vec<Object>),
    Exception(Exception),
}

/// A single entry inside the heap arena, storing refcount and payload.
#[derive(Debug)]
struct HeapObject {
    refcount: usize,
    data: HeapData,
}

/// Reference-counted arena that backs all heap-only runtime objects.
///
/// The heap never reuses IDs during a single execution; instead it appends new
/// entries and relies on `clear()` between runs.  This keeps identity checks
/// simple and avoids the need for generation counters while we're still
/// building out semantics.
#[derive(Debug)]
pub struct Heap {
    objects: Vec<Option<HeapObject>>,
}

impl Heap {
    /// Creates an empty heap ready to service allocations for a single executor run.
    pub fn new() -> Self {
        Self { objects: Vec::new() }
    }

    /// Allocates a new heap object, returning the fresh identifier.
    pub fn allocate(&mut self, data: HeapData) -> ObjectId {
        let id = self.objects.len();
        self.objects.push(Some(HeapObject { refcount: 1, data }));
        id
    }

    /// Increments the reference count for an existing heap object.
    pub fn inc_ref(&mut self, id: ObjectId) {
        let object = self
            .objects
            .get_mut(id)
            .unwrap_or_else(|| panic!("Heap::inc_ref: slot {id} missing"))
            .as_mut()
            .unwrap_or_else(|| panic!("Heap::inc_ref: object {id} already freed"));
        object.refcount += 1;
    }

    /// Decrements the reference count and frees the object (plus children) once it hits zero.
    pub fn dec_ref(&mut self, id: ObjectId) {
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            let slot = self
                .objects
                .get_mut(current)
                .unwrap_or_else(|| panic!("Heap::dec_ref: slot {current} missing"));
            let entry = slot
                .as_mut()
                .unwrap_or_else(|| panic!("Heap::dec_ref: object {current} already freed"));
            if entry.refcount > 1 {
                entry.refcount -= 1;
                continue;
            }

            let owned = slot.take().map(|owned| owned.data);
            if let Some(data) = owned {
                enqueue_children(&data, &mut stack);
            }
        }
    }

    /// Returns an immutable reference to the heap data stored at the given ID.
    pub fn get(&self, id: ObjectId) -> &HeapData {
        &self
            .objects
            .get(id)
            .unwrap_or_else(|| panic!("Heap::get: slot {id} missing"))
            .as_ref()
            .unwrap_or_else(|| panic!("Heap::get: object {id} already freed"))
            .data
    }

    /// Returns a mutable reference to the heap data stored at the given ID.
    pub fn get_mut(&mut self, id: ObjectId) -> &mut HeapData {
        &mut self
            .objects
            .get_mut(id)
            .unwrap_or_else(|| panic!("Heap::get_mut: slot {id} missing"))
            .as_mut()
            .unwrap_or_else(|| panic!("Heap::get_mut: object {id} already freed"))
            .data
    }

    /// Removes all objects and resets the ID counter, used between executor runs.
    pub fn clear(&mut self) {
        self.objects.clear();
    }
}

/// Pushes any child object IDs referenced by `data` onto the provided stack so
/// `dec_ref` can recursively drop entire object graphs without recursion.
fn enqueue_children(data: &HeapData, stack: &mut Vec<ObjectId>) {
    match data {
        HeapData::List(_items) | HeapData::Tuple(_items) => {
            // Non-heap references will be added in later phases; keep placeholders so the
            // match arms are ready once Object::Ref exists.
            let _ = stack;
        }
        HeapData::Exception(_exc) => {
            let _ = stack;
        }
        HeapData::Str(_) | HeapData::Bytes(_) => {}
    }
}

//! The payload of a secret value, wiped when dropped.

use std::ops::Deref;

use zeroize::Zeroize;

use super::value::REDACTED;
use super::{FileData, Value};

/// A value marked secret: the bytes it owns are zeroed when it is dropped.
///
/// Reads see through it by reference ([`Deref`] to the inner [`Value`]). There
/// is no `DerefMut`, since assigning through one would drop the old value
/// unwiped. A payload moved out through `TryFrom<Value>`, e.g. a `String`, is
/// the caller's and is not wiped; the secret keeps nothing of it.
pub struct Secret(
    /// Boxed, so moving a secret copies only a pointer.
    Box<Value>,
);

impl Secret {
    pub(crate) fn new(value: Value) -> Self {
        Secret(Box::new(value))
    }

    /// The inner value, for the writer to read file handles through. Assigning
    /// through it drops the replaced value unwiped.
    pub(crate) fn value_mut(&mut self) -> &mut Value {
        &mut self.0
    }

    /// Move a payload out of the innermost value, through any nested secrets,
    /// or hand the secret back intact when `detach` finds none.
    ///
    /// `detach` must move the payload out with `mem::take` or `mem::replace`,
    /// leaving an empty one, so the secret keeps nothing it handed over.
    pub(crate) fn take<Payload>(
        mut self,
        detach: impl FnOnce(&mut Value) -> Option<Payload>,
    ) -> Result<Payload, Value> {
        let mut innermost = &mut *self.0;
        while let Value::Secret(nested) = innermost {
            innermost = &mut *nested.0;
        }
        match detach(innermost) {
            Some(payload) => Ok(payload),
            None => Err(Value::Secret(self)),
        }
    }
}

impl Deref for Secret {
    type Target = Value;
    fn deref(&self) -> &Value {
        &self.0
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(REDACTED)
    }
}

/// Zeroize every byte `value` holds, spare capacity included. Variant tags,
/// dtypes, buffer pointers and capacities stay.
fn wipe(value: &mut Value) {
    match value {
        Value::Bool(b) => b.zeroize(),
        Value::Int(i) => i.zeroize(),
        Value::Float(x) => x.zeroize(),
        Value::String(s) => s.zeroize(),
        Value::Array(array) => {
            array.shape.zeroize();
            array.data.zeroize();
        }
        Value::File(file) => {
            file.mimetype.zeroize();
            match &mut file.data {
                FileData::Bytes(bytes) => bytes.zeroize(),
                // A handle's bytes stay with the caller's reader.
                FileData::Handle(_) => {}
            }
        }
        Value::Object(object) => {
            // Spare capacity can hold entries a pop or remove moved out.
            object.spare_capacity_mut().zeroize();
            for (key, entry) in object.iter_mut() {
                key.zeroize();
                wipe(entry);
            }
        }
        Value::List(items) => {
            items.spare_capacity_mut().zeroize();
            items.iter_mut().for_each(wipe);
        }
        // Wipes itself when dropped.
        Value::Secret(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use std::mem::{MaybeUninit, size_of_val};
    use std::{ptr, slice};

    use super::*;
    use crate::types::{Array, DType, File};

    /// The capacity of every buffer the test builds.
    const CAPACITY: usize = 32;

    /// `len` copies of `element`, with every slot up to `CAPACITY` written.
    fn filled<Element: Copy>(len: usize, element: Element) -> Vec<Element> {
        let mut buffer = Vec::with_capacity(CAPACITY);
        buffer.resize(len, element);
        buffer.spare_capacity_mut().fill(MaybeUninit::new(element));
        buffer
    }

    fn text(len: usize) -> String {
        String::from_utf8(filled(len, b's')).expect("ASCII is UTF-8")
    }

    /// `items` with spare slots whose every byte, padding included, is written.
    fn with_spare<Element>(mut items: Vec<Element>) -> Vec<Element> {
        items.reserve(2);
        let spare = items.spare_capacity_mut();
        // SAFETY: `MaybeUninit` slots accept any bytes.
        unsafe { ptr::write_bytes(spare.as_mut_ptr(), 0xA5, spare.len()) };
        items
    }

    fn assert_zeroed<Element: Copy + Default + PartialEq>(buffer: &mut Vec<Element>) {
        // A buffer swapped for an empty one would hide the bytes left in the old.
        assert_eq!(buffer.capacity(), CAPACITY);
        // SAFETY: `filled` wrote every slot up to `CAPACITY`, and the wipe only
        // overwrites them.
        unsafe { buffer.set_len(CAPACITY) };
        assert!(buffer.iter().all(|element| *element == Element::default()));
    }

    fn assert_spare_zeroed<Element>(items: &mut Vec<Element>) {
        let spare = items.spare_capacity_mut();
        assert!(!spare.is_empty());
        // SAFETY: `with_spare` wrote every byte of the spare slots, and the
        // wipe only overwrites them.
        let bytes =
            unsafe { slice::from_raw_parts(spare.as_ptr().cast::<u8>(), size_of_val(spare)) };
        assert!(bytes.iter().all(|byte| *byte == 0));
    }

    fn bytes_of(string: &mut String) -> &mut Vec<u8> {
        // SAFETY: every byte the test exposes is `s` or zero, both UTF-8.
        unsafe { string.as_mut_vec() }
    }

    fn assert_wiped(value: &mut Value) {
        match value {
            Value::Bool(b) => assert!(!*b),
            Value::Int(i) => assert_eq!(*i, 0),
            Value::Float(x) => assert_eq!(x.to_bits(), 0),
            Value::String(s) => assert_zeroed(bytes_of(s)),
            Value::Array(array) => {
                assert_zeroed(&mut array.shape);
                assert_zeroed(&mut array.data);
            }
            Value::File(file) => {
                assert_zeroed(bytes_of(&mut file.mimetype));
                match &mut file.data {
                    FileData::Bytes(bytes) => assert_zeroed(bytes),
                    FileData::Handle(_) => panic!("only in-memory files here"),
                }
            }
            Value::Object(object) => {
                assert_spare_zeroed(object);
                for (key, entry) in object.iter_mut() {
                    assert_zeroed(bytes_of(key));
                    assert_wiped(entry);
                }
            }
            Value::List(items) => {
                assert_spare_zeroed(items);
                items.iter_mut().for_each(assert_wiped);
            }
            Value::Secret(_) => panic!("a nested secret wipes itself on drop"),
        }
    }

    #[test]
    fn wipe_zeroes_every_byte_including_spare_capacity() {
        let mut value = Value::object(with_spare(vec![
            (text(4), Value::Int(5432)),
            (text(6), Value::String(text(8))),
            (
                text(5),
                Value::Array(Array::new(DType::U8, filled(1, 8), filled(8, 0xA5))),
            ),
            (
                text(4),
                Value::File(File::from_bytes(text(4), filled(8, 0xA5))),
            ),
            (
                text(3),
                Value::list(with_spare(vec![Value::Float(1.5), Value::Bool(true)])),
            ),
        ]));
        wipe(&mut value);
        assert_wiped(&mut value);
    }
}

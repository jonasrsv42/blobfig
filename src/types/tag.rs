//! Value type tag — the one-byte discriminant shared by the owned
//! [`Value`](super::Value) and borrowed [`ValueView`](super::ValueView).

/// Value type tags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ValueTag {
    Bool = 0x01,
    Int = 0x02,
    Float = 0x03,
    String = 0x04,
    Array = 0x05,
    File = 0x06,
    Object = 0x07,
    List = 0x08,
    /// Wraps another value, marking it secret: redacted when printed.
    Secret = 0x09,
}

impl ValueTag {
    pub fn from_u8(tag: u8) -> Option<Self> {
        match tag {
            0x01 => Some(ValueTag::Bool),
            0x02 => Some(ValueTag::Int),
            0x03 => Some(ValueTag::Float),
            0x04 => Some(ValueTag::String),
            0x05 => Some(ValueTag::Array),
            0x06 => Some(ValueTag::File),
            0x07 => Some(ValueTag::Object),
            0x08 => Some(ValueTag::List),
            0x09 => Some(ValueTag::Secret),
            _ => None,
        }
    }
}

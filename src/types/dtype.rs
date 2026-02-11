//! Data types for typed arrays

/// Data type for typed arrays
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DType {
    U8 = 0x01,
    I8 = 0x02,
    U16 = 0x03,
    I16 = 0x04,
    U32 = 0x05,
    I32 = 0x06,
    U64 = 0x07,
    I64 = 0x08,
    F32 = 0x09,
    F64 = 0x0A,
}

impl DType {
    /// Size in bytes of a single element
    pub fn element_size(self) -> usize {
        match self {
            DType::U8 | DType::I8 => 1,
            DType::U16 | DType::I16 => 2,
            DType::U32 | DType::I32 | DType::F32 => 4,
            DType::U64 | DType::I64 | DType::F64 => 8,
        }
    }

    /// Try to convert from u8 tag
    pub fn from_u8(tag: u8) -> Option<Self> {
        match tag {
            0x01 => Some(DType::U8),
            0x02 => Some(DType::I8),
            0x03 => Some(DType::U16),
            0x04 => Some(DType::I16),
            0x05 => Some(DType::U32),
            0x06 => Some(DType::I32),
            0x07 => Some(DType::U64),
            0x08 => Some(DType::I64),
            0x09 => Some(DType::F32),
            0x0A => Some(DType::F64),
            _ => None,
        }
    }
}

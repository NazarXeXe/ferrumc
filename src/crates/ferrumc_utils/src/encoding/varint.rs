use std::fmt::Display;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncWrite, AsyncWriteExt};

use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct VarInt {
    /// The value of the VarInt.
    val: i32,
    /// The length of the VarInt in bytes.
    len: usize,
}

impl VarInt {
    pub fn new(value: i32) -> Self {
        let bytes_required = if value < 128 && value >= -128 {
            1
        } else if value < 16384 && value >= -16384 {
            2
        } else if value < 2097152 && value >= -2097152 {
            3
        } else if value < 268435456 && value >= -268435456 {
            4
        } else {
            5
        };
        VarInt {
            val: value,
            len: bytes_required,
        }
    }
}

impl Display for VarInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.val)
    }
}

impl From<VarInt> for i32 {
    fn from(varint: VarInt) -> i32 {
        varint.val
    }
}

impl From<i32> for VarInt {
    fn from(value: i32) -> VarInt {
        VarInt::new(value)
    }
}

impl Into<usize> for VarInt {
    fn into(self) -> usize {
        self.val as usize
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[tokio::test]
    async fn read_varint_valid_input() {
        let mut cursor = Cursor::new(vec![0x80, 0x80, 0x80, 0x80, 0x08]);
        let result = read_varint(&mut cursor).await;
        assert_eq!(result.unwrap(), VarInt::new(-2147483648));
    }

    #[tokio::test]
    async fn read_varint_too_big() {
        let mut cursor = Cursor::new(vec![0b10000000; 6]);
        let result = read_varint(&mut cursor).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn write_varint_valid_input() {
        let mut cursor = Cursor::new(Vec::new());
        let result = write_varint(2097151, &mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(cursor.into_inner(), vec![0xff, 0xff, 0x7f]);
    }

    #[tokio::test]
    async fn write_varint_zero() {
        let mut cursor = Cursor::new(Vec::new());
        let result = write_varint(0, &mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(cursor.into_inner(), vec![0b00000000]);
    }

    #[tokio::test]
    async fn read_varint_empty_input() {
        let mut cursor = Cursor::new(vec![]);
        let result = read_varint(&mut cursor).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_varint_single_byte() {
        let mut cursor = Cursor::new(vec![0b00000001]);
        let result = read_varint(&mut cursor).await;
        assert_eq!(result.unwrap(), VarInt::new(1));
    }

    #[tokio::test]
    async fn write_varint_negative_input() {
        let mut cursor = Cursor::new(Vec::new());
        let result = write_varint(-1, &mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(cursor.into_inner(), vec![0xff, 0xff, 0xff, 0xff, 0x0f]);
    }
}

// Read a VarInt from the given cursor.
// Yoinked from valence: https://github.com/valence-rs/valence/blob/main/crates/valence_protocol/src/var_int.rs#L69
pub async fn read_varint<T>(cursor: &mut T) -> Result<VarInt, Error>
where
    T: AsyncRead + AsyncSeek + Unpin,
{
    let mut val = 0;
    for i in 0..5 {
        let byte = cursor.read_u8().await.map_err(|e| Error::Io(e))?;
        val |= (i32::from(byte) & 0b01111111) << (i * 7);
        if byte & 0b10000000 == 0 {
            return Ok(VarInt { val, len: i + 1 });
        }
    }
    Err(Error::Generic("VarInt is too big".to_string()))
}

// Write a VarInt to the given cursor.
// Yoinked from valence: https://github.com/valence-rs/valence/blob/main/crates/valence_protocol/src/var_int.rs#L98
pub async fn write_varint<A, T>(value: A, cursor: &mut T) -> Result<(), Error>
where
    A: Into<VarInt>,
    T: AsyncWrite + Unpin + AsyncSeek,
{
    let val = value.into().val as u64;

    let stage1 = (val & 0x000000000000007f)
        | ((val & 0x0000000000003f80) << 1)
        | ((val & 0x00000000001fc000) << 2)
        | ((val & 0x000000000fe00000) << 3)
        | ((val & 0x00000000f0000000) << 4);

    let leading = stage1.leading_zeros();

    let unused_bytes = (leading - 1) >> 3;
    let bytes_needed = 8 - unused_bytes;

    // set all but the last MSBs
    let most_significant_bits = 0x8080808080808080;
    let most_significant_bits_mask = 0xffffffffffffffff >> (((8 - bytes_needed + 1) << 3) - 1);

    let merged = stage1 | (most_significant_bits & most_significant_bits_mask);
    let bytes = merged.to_le_bytes();

    cursor.write_all(unsafe { bytes.get_unchecked(..bytes_needed as usize) }).await?;

    Ok(())
}

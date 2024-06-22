use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncWrite, AsyncWriteExt};

use crate::error::Error;

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::encoding::string::{read_string, write_string};

    #[tokio::test]
    async fn read_string_valid_input() {
        let mut data = b"Test".to_vec();
        data.insert(0, 4);
        let mut cursor = Cursor::new(data);
        println!("{:?}", cursor.clone().into_inner());
        let result = read_string(&mut cursor).await;
        assert_eq!(result.unwrap(), "Test");
    }

    #[tokio::test]
    async fn read_string_empty_input() {
        let mut cursor = Cursor::new(vec![]);
        let result = read_string(&mut cursor).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn write_string_valid_input() {
        let mut cursor = Cursor::new(Vec::new());
        let result = write_string("Test", &mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(
            cursor.into_inner(),
            vec![4, 0b01010100, 0b01100101, 0b01110011, 0b01110100]
        );
    }

    #[tokio::test]
    async fn write_string_empty_input() {
        let mut cursor = Cursor::new(Vec::new());
        let result = write_string("", &mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(cursor.into_inner(), vec![0b00000000]);
    }
}

pub async fn read_string<T>(cursor: &mut T) -> Result<String, Error>
where
    T: AsyncRead + Unpin + AsyncSeek,
{
    let length = crate::encoding::varint::read_varint(cursor).await?.into();
    let mut buffer = vec![0u8; length as usize];
    for i in 0..length {
        buffer[i as usize] = cursor
            .read_u8()
            .await
            .map_err(|_| Error::Generic("Bad byte read".parse().unwrap()))?;
    }
    Ok(String::from_utf8(buffer)?)
}

pub async fn write_string<T>(string: &str, cursor: &mut T) -> Result<(), Error>
where
    T: AsyncWrite + Unpin + AsyncSeek,
{
    let length = string.len();
    crate::encoding::varint::write_varint(length as i32, cursor).await
        .map_err(|_| Error::Generic("Failed to write varint".parse().unwrap()))?;
    cursor
        .write_all(string.as_bytes())
        .await
        .map_err(|_| Error::Generic("Failed to write string".parse().unwrap()))?;
    Ok(())
}

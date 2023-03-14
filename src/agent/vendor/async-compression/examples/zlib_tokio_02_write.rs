use async_compression::tokio_02::write::ZlibDecoder;
use async_compression::tokio_02::write::ZlibEncoder;

use std::io::Result;
use tokio_02::io::AsyncWriteExt as _; // for `write_all` and `shutdown`

use tokio_02 as tokio; // this enable the tokio main macro
#[tokio_02::main]
async fn main() -> Result<()> {
    let data = b"example";
    let compressed_data = compress(data).await?;
    let de_compressed_data = decompress(&compressed_data).await?;
    assert_eq!(de_compressed_data, data);
    println!("{:?}", String::from_utf8(de_compressed_data).unwrap());
    Ok(())
}

async fn compress(in_data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new());
    encoder.write_all(in_data).await?;
    encoder.shutdown().await?;
    Ok(encoder.into_inner())
}

async fn decompress(in_data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(Vec::new());
    decoder.write_all(in_data).await?;
    decoder.shutdown().await?;
    Ok(decoder.into_inner())
}

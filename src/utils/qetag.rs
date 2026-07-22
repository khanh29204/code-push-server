use base64::{Engine as _, engine::general_purpose::URL_SAFE};
use sha1::{Digest, Sha1};
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, BufReader};

const BLOCK_SIZE: usize = 4 * 1024 * 1024; // 4MB

pub async fn calc_qetag<P: AsRef<Path>>(
    file_path: P,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let file = File::open(file_path).await?;
    let mut reader = BufReader::with_capacity(BLOCK_SIZE, file);
    let mut sha1_blocks = Vec::new();
    let mut block_count = 0;

    let mut buffer = vec![0u8; BLOCK_SIZE];

    loop {
        let mut read_bytes = 0;
        while read_bytes < BLOCK_SIZE {
            let n = reader.read(&mut buffer[read_bytes..]).await?;
            if n == 0 {
                break;
            }
            read_bytes += n;
        }

        if read_bytes == 0 {
            break;
        }

        let mut hasher = Sha1::new();
        hasher.update(&buffer[..read_bytes]);
        sha1_blocks.extend_from_slice(&hasher.finalize());
        block_count += 1;

        if read_bytes < BLOCK_SIZE {
            break;
        }
    }

    if block_count == 0 {
        return Ok("Fto5o-5ea0sNMlW_75VgGJCv2AcJ".to_string());
    }

    let mut prefix = 0x16u8;
    let mut final_bytes = Vec::with_capacity(21);

    if block_count > 1 {
        prefix = 0x96u8;
        let mut final_hasher = Sha1::new();
        final_hasher.update(&sha1_blocks);
        final_bytes.push(prefix);
        final_bytes.extend_from_slice(&final_hasher.finalize());
    } else {
        final_bytes.push(prefix);
        final_bytes.extend_from_slice(&sha1_blocks);
    }

    Ok(URL_SAFE.encode(final_bytes))
}

/*
 * Copyright 2019 fsyncd, Berlin, Germany.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use nix::sys::socket::{SockAddr, VsockAddr};
use rand::RngCore;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_vsock::VsockStream;

const TEST_BLOB_SIZE: usize = 100_000;
const TEST_BLOCK_SIZE: usize = 5_000;

/// A simple test for the tokio-vsock implementation.
/// Generate a large random blob of binary data, and transfer it in chunks over the VsockStream
/// interface. The vm enpoint is running a simple echo server, so for each chunk we will read
/// it's reply and compute a checksum. Comparing the data sent and received confirms that the
/// vsock implementation does not introduce corruption and properly implements the interface
/// semantics.
#[tokio::test]
async fn test_vsock_server() {
    let mut rng = rand::thread_rng();
    let mut blob: Vec<u8> = vec![];
    let mut rx_blob = vec![];
    let mut tx_pos = 0;

    blob.resize(TEST_BLOB_SIZE, 0);
    rx_blob.resize(TEST_BLOB_SIZE, 0);
    rng.fill_bytes(&mut blob);

    let mut stream = VsockStream::connect(&SockAddr::Vsock(VsockAddr::new(3, 8000)))
        .await
        .expect("connection failed");

    while tx_pos < TEST_BLOB_SIZE {
        let written_bytes = stream
            .write(&blob[tx_pos..tx_pos + TEST_BLOCK_SIZE])
            .await
            .expect("write failed");
        if written_bytes == 0 {
            panic!("stream unexpectedly closed");
        }

        let mut rx_pos = tx_pos;
        while rx_pos < (tx_pos + written_bytes) {
            let read_bytes = stream
                .read(&mut rx_blob[rx_pos..])
                .await
                .expect("read failed");
            if read_bytes == 0 {
                panic!("stream unexpectedly closed");
            }
            rx_pos += read_bytes;
        }

        tx_pos += written_bytes;
    }

    let expected = Sha256::digest(&blob);
    let actual = Sha256::digest(&rx_blob);

    assert_eq!(expected, actual);
}

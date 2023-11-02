use anyhow::{anyhow, Context, Result};
use sendfd::SendWithFd;
use std::{
    fs::OpenOptions,
    os::fd::{AsRawFd, OwnedFd},
    os::unix::fs::OpenOptionsExt,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

// Note: the fd will be closed after passing
async fn passfd_connect(uds: &str, port: u32, fd: OwnedFd) -> Result<u32> {
    info!(sl!(), "passfd uds {:?} port {}", &uds, port);
    let mut stream = UnixStream::connect(&uds).await.context("connect")?;
    stream.write_all(b"passfd\n").await.context("write all")?;

    // We want the io connection keep connected when the containerd closed the io pipe,
    // thus it can be attached on the io stream.
    let buf = format!("{} keep", port);
    stream
        .send_with_fd(buf.as_bytes(), &[fd.as_raw_fd()])
        .context("send port and fd")?;

    let mut reads = BufReader::new(&mut stream);
    let mut response = String::new();
    reads.read_line(&mut response).await.context("read line")?;

    // parse response like "OK port"
    let mut iter = response.split_whitespace();
    if iter.next() != Some("OK") {
        return Err(anyhow!(
            "handshake error: malformed response code: {:?}",
            response
        ));
    }
    let hostport = iter
        .next()
        .ok_or_else(|| anyhow!("handshake error: malformed response code: {:?}", response))?
        .parse::<u32>()
        .context("handshake error: malformed response code")?;
    Ok(hostport)
}

#[derive(Debug, Default)]
pub struct PassfdIo {
    stdin: Option<String>,
    stdout: Option<String>,
    stderr: Option<String>,

    pub stdin_port: Option<u32>,
    pub stdout_port: Option<u32>,
    pub stderr_port: Option<u32>,
}

impl PassfdIo {
    pub async fn new(
        stdin: Option<String>,
        stdout: Option<String>,
        stderr: Option<String>,
    ) -> Self {
        Self {
            stdin,
            stdout,
            stderr,
            ..Default::default()
        }
    }

    pub async fn open_and_passfd(
        &mut self,
        uds_path: &str,
        passfd_port: u32,
        terminal: bool,
    ) -> Result<()> {
        // In linux, when a FIFO is opened and there are no writers, the reader
        // will continuously receive the HUP event. This can be problematic
        // when creating containers in detached mode, as the stdin FIFO writer
        // is closed after the container is created, resulting in this situation.
        //
        // See: https://stackoverflow.com/questions/15055065/o-rdwr-on-named-pipes-with-poll
        if let Some(stdin) = &self.stdin {
            let fin = OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(&stdin)
                .context("open stdin")?;

            let hostport = passfd_connect(uds_path, passfd_port, fin.into())
                .await
                .context("passfd")?;

            self.stdin_port = Some(hostport);
        }

        if let Some(stdout) = &self.stdout {
            let fout = OpenOptions::new()
                .write(true)
                .open(&stdout)
                .context("open stdout")?;

            let hostport = passfd_connect(uds_path, passfd_port, fout.into())
                .await
                .context("passfd")?;

            self.stdout_port = Some(hostport);
        }

        if !terminal {
            // stderr is not used in terminal mode
            if let Some(stderr) = &self.stderr {
                let ferr = OpenOptions::new()
                    .write(true)
                    .open(&stderr)
                    .context("open stderr")?;

                let hostport = passfd_connect(uds_path, passfd_port, ferr.into())
                    .await
                    .context("passfd")?;

                self.stderr_port = Some(hostport);
            }
        }

        Ok(())
    }
}

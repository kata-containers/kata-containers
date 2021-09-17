use std::process::Stdio;
use std::sync::Arc;
use tokio::sync::Mutex;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use protocols::image;
use ttrpc;

use crate::sandbox::Sandbox;

pub struct ImageService {
    sandbox: Arc<Mutex<Sandbox>>,
}

impl ImageService {
    pub fn new(sandbox: Arc<Mutex<Sandbox>>) -> Self {
        Self { sandbox }
    }

    async fn pull_image(&self, image: &str) -> Result<String> {
        let shell = std::path::PathBuf::from("/opt/pull_image.sh");
        let child = tokio::process::Command::new(shell)
            .arg(image)
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("pull image command failed to start");

        let join_handler = tokio::spawn(async move {
            match child.wait_with_output().await {
                Ok(output) => {
                    if output.status.success() {
                        //println!(
                        //    "child success {}",
                        //    String::from_utf8(output.stdout).unwrap()
                        //);
                        return Ok(String::from_utf8(output.stdout).unwrap());
                    } else {
                        //println!("child fail {}", String::from_utf8(output.stderr).unwrap());
                        return Err(anyhow!(
                            "child fail {}",
                            String::from_utf8(output.stderr).unwrap()
                        ));
                    }
                }
                Err(e) => {
                    //println!("child exec err. {:?}", e);
                    return Err(e.into());
                }
            }
        });

        let resp =
            tokio::time::timeout(std::time::Duration::from_secs(100), join_handler).await???;
        let mut sandbox = self.sandbox.lock().await;
        sandbox.images.insert(String::from(image), resp.clone());
        info!(
            slog_scope::logger(),
            "agent pull image success: {:?}", sandbox.images
        );
        return Ok(String::from(image));
    }
}

#[async_trait]
impl protocols::image_ttrpc::ImageService for ImageService {
    async fn list_images(
        &self,
        _ctx: &ttrpc::r#async::TtrpcContext,
        _req: image::ListImagesRequest,
    ) -> ttrpc::Result<image::ListImagesResponse> {
        Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
            ttrpc::Code::NOT_FOUND,
            "/containerd.task.v2.ImageService/ListImages is not supported".to_string(),
        )))
    }
    async fn image_status(
        &self,
        _ctx: &ttrpc::r#async::TtrpcContext,
        _req: image::ImageStatusRequest,
    ) -> ttrpc::Result<image::ImageStatusResponse> {
        Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
            ttrpc::Code::NOT_FOUND,
            "/containerd.task.v2.ImageService/ImageStatus is not supported".to_string(),
        )))
    }
    async fn pull_image(
        &self,
        _ctx: &ttrpc::r#async::TtrpcContext,
        req: image::PullImageRequest,
    ) -> ttrpc::Result<image::PullImageResponse> {
        match self.pull_image(req.get_image().get_image()).await {
            Ok(r) => {
                let mut resp = image::PullImageResponse::new();
                resp.image_ref = r;
                return Ok(resp);
            }
            Err(e) => {
                return Err(ttrpc::Error::Others(e.to_string()));
            }
        }
    }

    async fn remove_image(
        &self,
        _ctx: &ttrpc::r#async::TtrpcContext,
        _req: image::RemoveImageRequest,
    ) -> ttrpc::Result<image::RemoveImageResponse> {
        Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
            ttrpc::Code::NOT_FOUND,
            "/containerd.task.v2.ImageService/RemoveImage is not supported".to_string(),
        )))
    }
    async fn image_fs_info(
        &self,
        _ctx: &ttrpc::r#async::TtrpcContext,
        _req: image::ImageFsInfoRequest,
    ) -> ttrpc::Result<image::ImageFsInfoResponse> {
        Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
            ttrpc::Code::NOT_FOUND,
            "/containerd.task.v2.ImageService/ImageFsInfo is not supported".to_string(),
        )))
    }
}

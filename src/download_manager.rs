use anyhow::{Result, anyhow};
use reqwest::{Client, StatusCode};
use std::{fs::create_dir_all, path::PathBuf, sync::mpsc::Sender};
use tokio::runtime::Runtime;

use crate::events::Event;

pub struct DownloadManager {
    download_runtime: Runtime,
    client: reqwest::Client,
    mpsc_tx: Sender<Event>,
}

impl DownloadManager {
    pub fn new(mpsc_tx: Sender<Event>, download_runtime: Runtime) -> Self {
        Self {
            download_runtime,
            client: reqwest::Client::default(),
            mpsc_tx,
        }
    }

    pub fn download(&self, album_id: u32, tracks: Vec<(String, PathBuf)>) {
        let client = self.client.clone();
        let notify = self.mpsc_tx.clone();

        self.download_runtime.spawn(async move {
            let mut join_set = tokio::task::JoinSet::new();

            tracks.into_iter().for_each(|(download_url, file_path)| {
                join_set.spawn(download_track_async(
                    client.clone(),
                    file_path,
                    download_url,
                ));
            });

            if let Some(first_failure) = join_set
                .join_all()
                .await
                .into_iter()
                .find(|result| result.is_err())
            {
                notify
                    .send(Event::AlbumDownLoadFailed(
                        album_id,
                        first_failure.err().unwrap(),
                    ))
                    .unwrap();
            } else {
                notify.send(Event::AlbumDownloaded(album_id)).unwrap();
            }
        });
    }
}

async fn download_track_async(client: Client, path: PathBuf, download_url: String) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    let download_response = client.get(download_url).send().await?;

    match download_response.status() {
        StatusCode::OK => {
            create_dir_all(path.parent().unwrap())?;

            let bytes = download_response.bytes().await?;
            tokio::fs::write(&path, bytes).await?;

            Ok(())
        }
        // Bandcamp URLs eventually return a 410-Gone response when the download link is no longer valid
        _ => Err(anyhow!(
            "Download status code: {}",
            download_response.status()
        )),
    }
}

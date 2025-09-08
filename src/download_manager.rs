use anyhow::{Result, anyhow};
use reqwest::{StatusCode, blocking::Client};
use std::{
    fs::{File, create_dir_all},
    io::{Write, copy},
    path::PathBuf,
    sync::{Arc, mpsc::Sender},
    thread::spawn,
};

use crate::events::Event;

pub struct DownloadManager {
    client: Arc<Client>,
    mpsc_tx: Sender<Event>,
}

impl DownloadManager {
    pub fn new(mpsc_tx: Sender<Event>) -> Self {
        Self {
            client: Arc::new(Client::default()),
            mpsc_tx,
        }
    }

    pub fn download(&self, album_id: u32, tracks: Vec<(String, PathBuf)>) {
        let mpsc_tx = self.mpsc_tx.clone();
        let client = self.client.clone();

        spawn(move || {
            let download_results: Vec<Result<()>> = tracks
                .iter()
                .map(|(download_url, file_path)| download_track(&client, &file_path, &download_url))
                .collect();

            if let Some(first_failure) = download_results.into_iter().find(|result| result.is_err())
            {
                mpsc_tx.send(Event::AlbumDownLoadFailed(
                    album_id,
                    first_failure.err().unwrap(),
                ))
            } else {
                mpsc_tx.send(Event::AlbumDownloaded(album_id))
            }
        });
    }
}

fn download_track(client: &Client, path: &PathBuf, download_url: &str) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    let mut download_response = client.get(download_url).send()?;

    match download_response.status() {
        StatusCode::OK => {
            create_dir_all(path.parent().unwrap())?;
            let mut file = File::create(&path)?;

            copy(&mut download_response, &mut file)?;
            file.flush()?;

            Ok(())
        }
        // Bandcamp URLs eventually return a 410-Gone response when the download link is no longer valid
        _ => Err(anyhow!(
            "Download status code: {}",
            download_response.status()
        )),
    }
}

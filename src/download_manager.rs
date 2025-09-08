use anyhow::{Result, anyhow};
use reqwest::{StatusCode, blocking::Client};
use std::{
    fs::{File, create_dir_all},
    io::{Write, copy},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread::{JoinHandle, spawn},
};

use crate::events::Event;

pub struct DownloadManager {
    download_queue_tx: Sender<Album>,
}

struct Album {
    id: u32,
    tracks: Vec<(String, PathBuf)>,
}

impl DownloadManager {
    pub fn new(mpsc_tx: Sender<Event>) -> Self {
        let client = Arc::new(Client::default());
        let channel = mpsc::channel();
        let download_queue = Arc::new(Mutex::new(channel.1));
        spawn_download_thread(download_queue.clone(), client.clone(), mpsc_tx.clone());
        spawn_download_thread(download_queue.clone(), client.clone(), mpsc_tx.clone());
        Self {
            download_queue_tx: channel.0,
        }
    }

    pub fn download(&self, album_id: u32, tracks: Vec<(String, PathBuf)>) {
        self.download_queue_tx
            .send(Album {
                id: album_id,
                tracks: tracks,
            })
            .unwrap();
    }
}

// TODO: next step is to see how much faster downloads are when I can switch
// from serial track downloading to something concurrent with futures
fn spawn_download_thread(
    download_queue: Arc<Mutex<Receiver<Album>>>,
    client: Arc<Client>,
    notify: Sender<Event>,
) -> JoinHandle<()> {
    spawn(move || {
        loop {
            let album = download_queue.lock().unwrap().recv().unwrap();

            let download_results: Vec<Result<()>> = album
                .tracks
                .iter()
                .map(|(download_url, file_path)| download_track(&client, &file_path, &download_url))
                .collect();

            if let Some(first_failure) = download_results.into_iter().find(|result| result.is_err())
            {
                notify
                    .send(Event::AlbumDownLoadFailed(
                        album.id,
                        first_failure.err().unwrap(),
                    ))
                    .unwrap();
            } else {
                notify.send(Event::AlbumDownloaded(album.id)).unwrap();
            }
        }
    })
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

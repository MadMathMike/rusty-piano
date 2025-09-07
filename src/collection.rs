use anyhow::{Result, anyhow};
use ratatui::prelude::*;
use ratatui::widgets::{Block, List, ListState, StatefulWidget, Widget};
use reqwest::StatusCode;
use std::fs::{File, create_dir_all};
use std::io::{Write, copy};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use crate::bandcamp;
use crate::events::Event;

pub struct Collection {
    albums: Vec<Album>,
    pub album_state: ListState,
}

impl Collection {
    pub fn from_bandcamp_items(bandcamp_items: Vec<crate::bandcamp::Item>) -> Self {
        let mut album_state = ListState::default();
        album_state.select(Some(0));

        let albums = bandcamp_items.into_iter().map(Album::from).collect();

        Self {
            albums,
            album_state,
        }
    }

    pub fn download_selected_album(&mut self, mpsc_tx: mpsc::Sender<Event>) -> Option<Album> {
        let selected = self.album_state.selected();
        if selected.is_none() {
            return None;
        }
        let selected = selected.unwrap();

        let album = self.albums.get_mut(selected);

        album.and_then(|album| match album.download_status {
            DownloadStatus::Downloading => None,
            DownloadStatus::Downloaded => Some(album.clone()),
            DownloadStatus::NotDownloaded | DownloadStatus::DownloadFailed => {
                album.download(mpsc_tx);
                None
            }
        })
    }

    // TODO: Holy shit this is so bad, for so many reasons.
    pub fn download_all(&mut self, mpsc_tx: mpsc::Sender<Event>) {
        self.albums
            .iter_mut()
            .map(|album| album.download(mpsc_tx.clone()))
            .for_each(|_| {});
    }

    pub fn get_album_mut(&mut self, id: u32) -> Option<&mut Album> {
        self.albums.iter_mut().find(|album| album.id == id)
    }
}

impl Widget for &mut Collection {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let album_titles = self
            .albums
            .iter()
            .map(|album| {
                let icon = match album.download_status {
                    DownloadStatus::NotDownloaded => '💾',
                    DownloadStatus::Downloading => '⏳',
                    DownloadStatus::Downloaded => '✅',
                    DownloadStatus::DownloadFailed => '🚨',
                };
                format!("{} {icon}", album.title.clone())
            })
            .collect::<Vec<String>>();

        let list = List::new(album_titles)
            .block(Block::bordered().title("Albums"))
            .highlight_symbol(">");

        StatefulWidget::render(list, area, buf, &mut self.album_state);
    }
}

#[derive(Clone)]
pub struct Album {
    pub id: u32,
    pub title: String,
    pub tracks: Vec<Track>,
    pub band_name: String,
    pub download_status: DownloadStatus,
}

impl Album {
    fn download(
        &mut self,
        mpsc_tx: mpsc::Sender<Event>,
    ) -> Option<JoinHandle<std::result::Result<(), mpsc::SendError<Event>>>> {
        match self.download_status {
            DownloadStatus::Downloaded | DownloadStatus::Downloading => None,
            DownloadStatus::NotDownloaded | DownloadStatus::DownloadFailed => {
                self.download_status = DownloadStatus::Downloading;
                let tracks = self.tracks.clone();
                let album_id = self.id;

                let handle = thread::spawn(move || {
                    let download_failure = tracks
                        .iter()
                        .map(|track| download_track(&track.file_path, &track.download_url))
                        .find(|result| result.is_err());

                    match download_failure {
                        None => mpsc_tx.send(Event::AlbumDownloaded(album_id)),
                        Some(err) => {
                            mpsc_tx.send(Event::AlbumDownLoadFailed(album_id, err.err().unwrap()))
                        }
                    }
                });

                Some(handle)
            }
        }
    }
}

fn download_track(path: &PathBuf, download_url: &str) -> Result<()> {
    // TODO: should I only have one client?
    let mut download_response = reqwest::blocking::Client::new().get(download_url).send()?;

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

impl From<bandcamp::Item> for Album {
    fn from(value: bandcamp::Item) -> Self {
        let tracks = value
            .tracks
            .iter()
            .map(|track| Track {
                number: track.track_number,
                title: track.title.clone(),
                download_url: track.hq_audio_url.clone(),
                file_path: to_file_path(&value, &track),
            })
            .collect::<Vec<Track>>();

        // figure out if everything is downloaded
        let download_status = if tracks.iter().all(|t| t.file_path.exists()) {
            DownloadStatus::Downloaded
        } else {
            DownloadStatus::NotDownloaded
        };

        Album {
            id: value.tralbum_id,
            title: format!("{} by {}", value.title, value.band_info.name),
            tracks,
            band_name: value.band_info.name,
            download_status,
        }
    }
}

fn to_file_path(album: &bandcamp::Item, track: &bandcamp::Track) -> PathBuf {
    // This is incomplete, but works for now as it address the one album in my library with a problem:
    // The album "tempor / jester" by A Unicorn Masquerade resulted in a jester subdirectory
    fn filename_safe_char(char: char) -> bool {
        char != '/'
    }

    let mut band_name = album.band_info.name.clone();
    band_name.retain(filename_safe_char);
    let mut album_title = album.title.clone();
    album_title.retain(filename_safe_char);
    // TODO: download path should definitely be partially built by some safe absolute path, (and hopefully) configurable
    let mut path = PathBuf::from(format!("./bandcamp/{band_name}/{album_title}"));

    let mut track_title = track.title.clone();
    track_title.retain(filename_safe_char);
    path.push(format!("{:02} - {track_title}.mp3", track.track_number));
    path
}

#[derive(PartialEq, Clone)]
pub enum DownloadStatus {
    NotDownloaded,
    Downloading,
    Downloaded,
    DownloadFailed,
}

#[derive(Clone)]
pub struct Track {
    pub number: u8,
    pub title: String,
    pub download_url: String,
    pub file_path: PathBuf,
}

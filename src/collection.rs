use ratatui::prelude::*;
use ratatui::widgets::{Block, List, ListState, StatefulWidget, Widget};
use std::path::PathBuf;

use crate::bandcamp;
use crate::download_manager::DownloadManager;

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

    pub fn download_selected_album(
        &mut self,
        download_manager: &DownloadManager,
    ) -> Option<&Album> {
        self.album_state
            .selected()
            .and_then(|index| self.albums.get_mut(index))
            .take_if(|a| a.download(download_manager))
            .map(|a| &*a)
            .or(None)
    }

    pub fn download_all(&mut self, download_manager: &DownloadManager) {
        self.albums.iter_mut().for_each(|album| {
            album.download(download_manager);
        });
    }

    pub fn set_downloaded(&mut self, id: u32) -> Option<&Album> {
        if let Some(album) = self.albums.iter_mut().find(|album| album.id == id) {
            album.download_status = DownloadStatus::Downloaded;
            Some(album)
        } else {
            None
        }
    }

    pub fn set_failed(&mut self, id: u32) -> Option<&Album> {
        if let Some(album) = self.albums.iter_mut().find(|album| album.id == id) {
            album.download_status = DownloadStatus::DownloadFailed;
            Some(album)
        } else {
            None
        }
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
    fn download(&mut self, download_manager: &DownloadManager) -> bool {
        match self.download_status {
            DownloadStatus::Downloading => false,
            DownloadStatus::Downloaded => true,
            DownloadStatus::NotDownloaded | DownloadStatus::DownloadFailed => {
                self.download_status = DownloadStatus::Downloading;

                let tracks = self
                    .tracks
                    .iter()
                    .map(|track| (track.download_url.clone(), track.file_path.clone()))
                    .collect();

                download_manager.download(self.id, tracks);

                false
            }
        }
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

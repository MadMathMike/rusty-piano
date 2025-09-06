use std::path::PathBuf;

pub mod app;
pub mod bandcamp;
pub mod json_l;
pub mod player;

impl From<bandcamp::Item> for app::Album {
    fn from(value: bandcamp::Item) -> Self {
        let tracks = value
            .tracks
            .iter()
            .map(|track| app::Track {
                number: track.track_number,
                title: track.title.clone(),
                download_url: track.hq_audio_url.clone(),
                file_path: to_file_path(&value, &track),
            })
            .collect::<Vec<app::Track>>();

        // figure out if everything is downloaded
        let download_status = if tracks.iter().all(|t| t.file_path.exists()) {
            app::DownloadStatus::Downloaded
        } else {
            app::DownloadStatus::NotDownloaded
        };

        app::Album {
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

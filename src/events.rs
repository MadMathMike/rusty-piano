use crossterm::event::KeyEvent;

pub enum Event {
    Input(KeyEvent),
    AlbumDownloaded(u32),
    AlbumDownLoadFailed(u32, anyhow::Error),
}

# Description
A terminal app (built with Rust) that can play collected albums from your ~~Pandora~~ Bandcamp music account.

# Goals
- [x] Play local mp3 on default media output device
- [x] Authenticate with ~~Pandora~~ Bandcamp API
- [x] Find a way to store/re-use auth-token
- [x] Request album list
- [x] Play single song from album
- [x] Pull down (and cache?) entire collection (must use paging)
- [x] Play songs from album consecutively
- [x] Add download all functionality
  - [x] ~Not like this... 😞~ We did it with tokio! 😀
- [ ] Support configurable download location
- [x] UI: Download state
  - [x] Show when album is downloaded
  - [x] Show when album is downloading
  - [x] Show when album download failed
- [x] UI: View albums
- [x] UI: Playback controls
  - [x] Play/pause (space bar?)
  - [x] Track navigation
- [x] UI: Show currently playing album
- [x] UI: Show currently playing song
- [x] UI: Navigation while album playing
- [x] UI: Controls
- [x] UI: Inlude artist in album list
- [ ] Update README
- [ ] Code docs

# Stretch Goals
- [ ] Autoplay next album
- [ ] Sort albums
- [ ] Song progress
- [ ] View albums by artist
- [ ] Preview songs in album before playing
- [ ] Search
- [ ] Re-auth flow
- [ ] Dynamically parse BandCamp responses to error model vs happy-path model
- [ ] Downloading progress bar (for slow connections)
- [ ] Debug logging for trouble shooting?

# Lessons
### Playback
Tried playback provided via [playback-rs](https://crates.io/crates/playback-rs/0.4.4).
 - Seems to require libasound2-dev (Ubuntu only?)
   - https://askubuntu.com/a/1396206
 - Seems to also require nightly/beta rust compiler (which I don't want to do)
   - https://askubuntu.com/a/1396206
```
   Compiling playback-rs v0.4.4
error[E0554]: `#![feature]` may not be used on the stable release channel
```

Switched to [rodio](https://crates.io/crates/rodio).
 - Similar dependencies: https://github.com/RustAudio/rodio?tab=readme-ov-file#dependencies-linux-only

### Keyring
Crate:
 - Seems to require dbus-devel

### Bandcamp
Thank the gods for this guy: https://mijailovic.net/2024/04/04/bandcamp-auth/ and https://github.com/Metalnem/bandcamp-downloader

## Archive
### Pandora
Pandora web app uses REST API: https://6xq.net/pandora-apidoc

Using cookies and setting a user-agent header help prevent xperimeter from flagging as a bot and issuing Captcha challenge
(ChatGPT really helped me figure this out)

Useful webtools filter `-collector -stats -/radio-health -getVersion -tzm.px-cloud.net -ingest.sentry.io`

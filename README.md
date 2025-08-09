# Description
Goal: build a terminal app using Rust that can play collected albums from your Pandora music account.

# Goals
 - [x] Play local mp3 on default media output device
 - [x] Authenticate with Pandora API
   - [x] ~Partner login~ Switched to REST API so this isn't needed
   - [x] ~Blowfish cipher~
   - [x] User authentication
 - [ ] Find a way to store/re-use auth-token to prevent getting flagged as a bot?
 - [ ] Securely store user credentials in system wallet? (After UI is implemented, probably)
 - [ ] Request album list
 - [ ] Play single song from album
 - [ ] Play songs from album consecutively
 - [ ] UI: View albums
 - [ ] UI: View songs in album
 - [ ] UI: Playback controls 
 - [ ] UI: Song progress
 - [ ] UI: Navigation while album playing
 - [ ] Stretch: Code: create macro to derive "pandora_response" (to auto-implement "stat" and "result" fields)
 - [ ] Stretch: Offline mode
 - [ ] Stretch: Local playlists
 - [ ] Stretch: Downloading progress bar (for slow connections)
 - [ ] Stretch: Debug logging for trouble shooting?
 - [ ] Stretch: Album/Song searching and collecting

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

### Pandora
Pandora web app uses REST API: https://6xq.net/pandora-apidoc

Using cookies and setting a user-agent header help prevent xperimeter from flagging as a bot and issuing Captcha challenge
(ChatGPT really helped me figure this out)
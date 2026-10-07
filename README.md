# MSG

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/msg_icon_white_256.png">
  <img src="img/msg_icon_black_256.png" width="120" align="right" alt="MSG logo">
</picture>

A desktop slideshow for booru sites: e621, Danbooru, Gelbooru, Rule34, Derpibooru, Tantabus and your own folders of images and videos. Search with tags, watch the results as a slideshow, keep favorites, save pools (comics), download, and use hotkeys.

MSG is a native Windows program written in Rust, with its window drawn by [egui](https://github.com/emilk/egui). It is based on version 10.6 of Chirmaya's [BooruSlideshow](https://github.com/Chirmaya/BooruSlideshow) browser extension: the slideshow, the site searches and the favorites come from it, ported from JavaScript to Rust.

The name: MSG, monosodium glutamate, is food additive E621. The logo (`img/msg_logo.svg`, and `img/msg_logo_small.svg` for 32 px and below) is its structural formula.

## Features

- **Search** the selected sites with tags, with a history, a sort menu and quick searches. Each quick search is a card on the front page with the first picture of its results and its main tags.
- **Slideshow** with automatic advance, preloading of the next slides, a strip of the next slides, and the tags of the current image grouped by category. Clicking a tag searches for it.
- **Your folders**: browse your own images and videos as a site. The search words filter by folder and file name (`-word` leaves out).
- **Favorites**: import them from e621, Derpibooru and Gelbooru, filter by tag or site, show them in random order, and keep e621 favorites in sync.
- **e621 pools** (comics): search `pool:17870` or paste a pool link and save the pool. The Pools page (📚 Pools or P) fills the window with the saved pools' covers, with a name filter and sorting; click a cover to read the pool. **Find pools in favorites** adds the pools that your e621 favorites are in.
- **Downloads** of the image or video on the screen, of all favorites, and in the background as offline copies of the favorites and saved pools that are used instead of the site when they are on the disk.
- **Video** through [libmpv](https://mpv.io/) (see below).
- **Rebindable hotkeys** and a settings page (⚙) that fills the window, with sections for appearance, sites and accounts, slideshow, filtering, hotkeys, folders, favorites, quick searches and history.

Rule34 needs a user ID and an API key (rule34.xxx → My Account → Options), and e621, Derpibooru, Gelbooru and Tantabus can use one for your own accounts and filters. They are entered in the settings.

## Install

Download `MSG-windows-x64.exe` from [Releases](../../releases). It is a single exe with no installer.

Videos are played with libmpv, loaded when the program starts: put `mpv-2.dll` (64-bit; for example from the `mpv-dev-lgpl-x86_64` package of [zhongfly/mpv-winbuild](https://github.com/zhongfly/mpv-winbuild)) next to the exe. Without it everything else works and a video slide says the library is missing.

Settings and favorites are kept in `%APPDATA%\MSG\settings.json`. They are not part of the download, so a new computer starts empty. Settings saved before the rename (in `%APPDATA%\booruslideshowelectron`) are copied over on the first start.

## Terminal version

`msg-cli.exe` is a lighter MSG for the terminal: e621 searches and your favorites as a slideshow of pictures (no videos), with the same hotkeys, favorites (and e621 sync), tags, pools, downloads and quick searches. It reads and writes the same `settings.json`; the settings themselves are changed in the MSG window.

```
msg-cli [--sixel | --blocks] [tags…]
```

In Windows Terminal (1.22 or newer) the pictures are drawn with Sixel, at about the screen's own resolution in full screen (F11; U hides the bars), with up to 256 colours for every strip of three text rows. Other terminals get half blocks (two 24-bit colour pixels per character). `--sixel` and `--blocks` choose by hand. Press `?` for the keys. GIFs show their first frame.

## Building

Requires [Rust](https://rustup.rs/) and the Visual Studio C++ Build Tools. Close MSG before building, or the build fails.

```
cargo run --release     # run
cargo build --release   # target/release/MSG.exe and msg-cli.exe
cargo run --release --bin msg-cli -- <tags>
cargo test
```

Pushing a tag like `v2.0.0` runs `.github/workflows/build.yml`, which builds the exe on GitHub and attaches it to that release.

For testing, `MSG_SEARCH=<tags>` makes MSG search at start, and `MSG_SHOT=<file.png>` saves a screenshot after `MSG_SHOT_AFTER` seconds (default 12) and exits.

## History

The earlier versions are in the git history: the Electron version (tag `electron-final`) and the Tauri version with the web view and its glass look (tag `webview-final`). Neither is developed any more.

## License and credits

MSG is based on version 10.6 of [BooruSlideshow](https://github.com/Chirmaya/BooruSlideshow) by Chirmaya, whose license (see `LICENSE`) is:

> Copyright (c) 2026 Chirmaya
>
> Open license to copy/modify/etc. as long as you:
> - Don't publish under the name "Booru Slideshow" (to reduce name confusion)

That is why this app is called MSG and has its own icons. The license text and credits are also in Settings → About, along with michutsu's [BooruSlideshowElectron](https://github.com/michutsu/BooruSlideshowElectron) (an earlier Electron port; no code from it is used). MSG uses [egui](https://github.com/emilk/egui) and eframe (MIT or Apache-2.0) and the Rust libraries under it (reqwest, tokio and others, each MIT or Apache-2.0). For video it loads [libmpv](https://mpv.io/) (LGPL-2.1 or later) at run time; it is not included in the download.

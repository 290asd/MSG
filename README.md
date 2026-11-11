# MSG

<img src="img/msg_icon_256.png" width="120" align="right" alt="MSG logo">

A quiet slideshow for booru sites. Search, look, keep what you like. The pictures come first, and everything else stays out of the way until you need it.

Sites: e621, Danbooru, Gelbooru, Rule34, Derpibooru, Tantabus and your own folders.

Native Windows, written in Rust with [egui](https://github.com/emilk/egui). Based on Chirmaya's [BooruSlideshow](https://github.com/Chirmaya/BooruSlideshow) 10.6.

*MSG, monosodium glutamate, is food additive E621. The logo is its molecule.*

## Features

- **Mosaic**: every search opens as a wall of pictures. Click one to look closer, or press Space to play.
- **Slideshow**: the picture fills the window. The controls fade away, and the rest waits in a drawer on the left.
- **Reading list**: follow artists and tags, and their newest posts are your front page.
- **Hottest**: the best of the last day, week, month or year.
- **Favorites**: import them from e621, Derpibooru and Gelbooru, and keep them in sync with e621.
- **Pools**: save comics and read them page by page.
- **Your folders**: your own pictures and videos, shown like any other site.
- **Downloads**, **video** (libmpv) and **rebindable hotkeys**.

Rule34 needs an API key; the other sites can use one for your own account. Add them in Settings.

## Install

Download `MSG-windows-x64.exe` from [Releases](../../releases). One exe, no installer.

For video, put `mpv-2.dll` next to it (64-bit, for example `mpv-dev-lgpl-x86_64` from [zhongfly/mpv-winbuild](https://github.com/zhongfly/mpv-winbuild)).

Settings and favorites are kept in `%APPDATA%\MSG\settings.json`.

## Terminal version

`msg-cli.exe` brings e621 to the terminal, drawn in Sixel in Windows Terminal and in half blocks elsewhere. It shares the hotkeys and favorites with MSG. Press `?` for the keys.

```
msg-cli [--sixel | --blocks] [tags…]
```

## Building

Needs [Rust](https://rustup.rs/) and the Visual Studio C++ Build Tools. Close MSG first.

```
cargo run --release
cargo test
```

A `vX.Y.Z` tag builds the release on GitHub.

## History

The Electron version (tag `electron-final`) and the Tauri version (tag `webview-final`) are kept in the git history.

## License and credits

Based on [BooruSlideshow](https://github.com/Chirmaya/BooruSlideshow) 10.6 by Chirmaya:

> Copyright (c) 2026 Chirmaya
>
> Open license to copy/modify/etc. as long as you:
> - Don't publish under the name "Booru Slideshow" (to reduce name confusion)

That is why MSG has its own name. It also thanks michutsu's [BooruSlideshowElectron](https://github.com/michutsu/BooruSlideshowElectron) (no code from it is used). It includes the [Inter](https://rsms.me/inter/) font (OFL 1.1), [Lucide](https://lucide.dev) icons (ISC) and [egui](https://github.com/emilk/egui) (MIT/Apache-2.0). It loads [libmpv](https://mpv.io/) (LGPL-2.1+) at run time, and libmpv is not included in the download. The full credits are in Settings → About.

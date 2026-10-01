# MSG

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="frontend/img/msg_icon_white_256.png">
  <img src="frontend/img/msg_icon_black_256.png" width="120" align="right" alt="MSG logo">
</picture>

A desktop slideshow for booru sites: e621, Danbooru, Gelbooru, Rule34, Realbooru, Derpibooru, Tantabus and your own folders of images and videos. Search with tags, watch the results as a slideshow, keep favorites, save pools (comics), download, and work with hotkeys or a touch screen.

It is based on version 10.6 of Chirmaya's [BooruSlideshow](https://github.com/Chirmaya/BooruSlideshow) browser extension and replaces the old [BooruSlideshowElectron](https://github.com/michutsu/BooruSlideshowElectron) port. **MSG runs on Windows only.**

The name: MSG, monosodium glutamate, is food additive E621. The logo (`img/msg_logo.svg`, and `img/msg_logo_small.svg` for 32 px and below) is its structural formula, black on the light theme and white on the dark one.

## Two versions

Both are built from the same Rust package in `src-tauri/` and share `settings.json`, so favorites and settings are the same in either.

| | MSG (`MSG.exe`) | MSG-native (`MSG-native.exe`) |
|---|---|---|
| Window | The extension's pages in the system's web view (Microsoft Edge WebView2), on [Tauri](https://tauri.app/) | Drawn with [egui](https://github.com/emilk/egui), no web view |
| Features | Everything in this file | The main ones: search, slideshow, tags, favorites, pools, quick searches, downloads, offline copies, your folders, hotkeys, settings |
| Left out | | Realbooru, joi, the glass/Refract effects, tag analysis, touch mode, "Find pools in favorites" |
| Video | The web view's own player | [libmpv](https://mpv.io/), loaded at start (see below) |
| Packaged as | Installer (`-setup.exe`, `.msi`) | A plain `.exe` |

## Install

Download from [Releases](../../releases).

- **MSG**: the `-setup.exe` installer (or the `.msi`). MSG shows its pages with the system's web view instead of bundling a browser, so the packages are small. It needs Microsoft Edge WebView2, which Windows 11 has; the installer fetches it on Windows 10 if it is missing.
- **MSG-native**: `MSG-native-windows-x64.exe`, no installer. Videos are played with libmpv: put `mpv-2.dll` (64-bit; for example from the `mpv-dev-lgpl-x86_64` package of [zhongfly/mpv-winbuild](https://github.com/zhongfly/mpv-winbuild)) next to the exe. Without it everything else works and a video slide says the library is missing.

Settings and favorites are kept in the user data folder `%APPDATA%\MSG`. They are not part of the packages, so a new computer starts empty. Features that are off by default (joi, the sort menu, Refract, offline copies) are turned on in the settings.

## Running and building

Requires [Rust](https://rustup.rs/), the Visual Studio C++ Build Tools and [Node.js](https://nodejs.org/). Close MSG before building, or the build fails.

```
npm install
npm start                                   # MSG in debug mode
npm run build                               # MSG alone: src-tauri/target/release/MSG.exe
npm run tauri build                         # the installers too (src-tauri/target/release/bundle/)

cd src-tauri
cargo run --release --bin MSG-native        # MSG-native
```

The pages (`frontend/`) are inside the app, so restart it to see a change there. Pushing a tag like `v1.0.0` runs `.github/workflows/build.yml`, which builds the Windows packages and `MSG-native` on GitHub and attaches them to that release.

## The web view version (MSG)

The pages are in `frontend/`; paths such as `js/`, `css/` and `img/` in this file are relative to it. The extension's logic (`js/`) comes from the original. `js/tauri_shim.js` provides the `chrome.*` API calls it uses, on top of the Rust commands in `src-tauri/src/`:

| Extension API | Replacement |
|---|---|
| `chrome.storage.sync` / `local` | `settings.json` in the app's user data folder |
| `chrome.downloads.download` | Saved to the download folder (Settings → Folders, default `Downloads/MSG/downloads/`) |
| `rules_header_referer.json` (Gelbooru Referer) | Images and videos load through the `msg-proxy` scheme (`src-tauri/src/net.rs`), which sets the Referer that Gelbooru, e621, Rule34 and Realbooru require |
| host permissions | XHR and `fetch` to the sites go through Rust (`http_request`), so the page reaches booru APIs without CORS |
| `window.open` / external links | Open in your default browser |

Settings and favorites from the old BooruSlideshowElectron are imported automatically on first start.

A background image for the start screen, and how much it is blurred, can be chosen in Settings → Appearance.

## Additions to the extension

**Look and settings**

- New "liquid glass" look (`css/app.css`): the image or video fills the window with nothing over it. Scrolling down brings up the search, then the navigation with one row of the next slides (as many as fit), and the tags, sliding over the image.
- Dark, light or system theme, and how much the glass blurs (Settings → Appearance). While a blur slider is moved, the settings window steps aside so the change can be seen.
- Tips about the app's features while an image loads (can be turned off in Settings → Appearance).
- Settings window (⚙) with sections for appearance, sites and accounts, slideshow, filtering, touch screen, hotkeys, history, favorites and about.
- Rebindable hotkeys (Settings → Hotkeys). New ones: Ctrl+F opens the favorites (and goes back), Ctrl+S opens the settings, Ctrl+L makes the image being shown the background image. These (and the tag key) can also be set to Ctrl/Shift/Alt combinations; the extension's own hotkeys take single keys, and combinations never trigger them.
- Tags of the current image (# button or R), laid out like e621: grouped by category in e621's order and colors, on the left over the image or at the bottom of the page. Categories come from e621 and Danbooru; other sites show one list. Clicking a tag searches for it.
- Touch screen mode: tap the left or right edge for previous/next, the middle to jump between the image and the controls, double-tap to fave.
- Play videos to the end before moving to the next slide, play videos automatically, start them muted (Settings → Slideshow).
- **Your folders** (Settings → Folders): browse your own images and videos as a site ("Your folders" in Sites & accounts). The search words filter by folder and file name (`-word` leaves out); an empty search shows everything. Subfolders are included. The same tab sets the download folder (default `Downloads/MSG`).
- Developer settings: click the logo in Settings → About five times. A live report of the image or video on the screen (site, file name, format, file size, source, natural and displayed frame size, video state), the window and the session, with a copy button, plus an FPS counter and the background download's progress in the top right corner, and logging to the console.
- Effects: Glass, or Solid, which turns off the blur and the animations (the loading orb keeps turning).
- The search bar on the start page has a rainbow border beam until the first search; an image that is a favorite gets the same beam around it.
- Quick searches on keys 1–0 (Settings → Quick searches): each keeps sites and, if wanted, tags; pressing the key selects those sites and searches (for its tags, or for what is in the search box).
- Quick search cards on the front page: each quick search is a card of its own, two to a row, with the first picture of its results, the search and the main tags (artist, character, series: e621 and Danbooru only). Click a card to run the search. Turn them off in Settings → Appearance.
- U brings up the controls (the search at the top of the window); again goes back to the image. No scrollbars.
- Rule34 works again through its new API, with the user ID and API key under Sites & accounts (rule34.xxx → My Account → Options).
- Realbooru is a site too (`js/objects/site_managers/site_manager_realbooru.js`). Its API is switched off, so MSG reads the site's pages instead: no login needed, but a page of 42 takes about 10 seconds (the site answers 503 if asked faster), the rating filter doesn't apply, and it can break if the site's pages change.
- Tantabus (tantabus.ai) is a site too. It runs the same software as Derpibooru, so it uses the same code (`site_manager_tantabus.js`). No login needed; an API key under Sites & accounts is optional and only matters for your own filters.

These live in `js/app_settings.js`. It changes hotkeys by setting the key constants in `globals.js`, which the views read on every key press.

**Favorites**

- **Import** (Settings → Favorites) adds your favorites from e621 (username), Derpibooru (API key) and Gelbooru (user ID and API key), using the details under Sites & accounts. Already-added items are skipped, so it can be run again to pick up new favorites.
- Faving or unfaving shows a heart (white when unfaved), and the card with the next slides is outlined with "♥ In favorites" while the image is a favorite. The search bar stays at the top of the window when scrolled past.
- **Also fave on e621** (Settings → Favorites, on by default): faving or unfaving an e621 image in the app does the same on your e621 account. Needs the e621 username and API key under Sites & accounts. Only failures are told.
- **Download all favorites** (Settings → Favorites) saves the favorites being shown (all, filtered or random) to `Downloads/MSG/favorites`, 3 at a time. Files already there are skipped, so a cancelled download can be resumed.
- The **💾 Download** button in the toolbar (both pages; hide it under Settings → Appearance) saves the image or video on the screen to the download folder, like the download hotkey.
- **Clicking a tag** (Settings → Appearance → Tags) searches or filters for that tag alone, or adds it to what is in the box.
- The **site menu** next to the filter box shows favorites from one site only (with a count for each). It works together with the tag filter, Random and Download all favorites.
- **♥ joi** (test, turn on in Settings → Favorites): in the search bar and on the favorites page, opens [joi.how](https://github.com/clynamic/joi.how)'s settings over the page, with the search results loaded so far or the favorites being shown (all, filtered or random) as its images; Begin starts the game there, Esc or × closes it. The `joi` folder is a build of joi.how (GPL-3.0, see `joi/README.md`, `joi/LICENSE` and MSG's changes in `joi/msg-changes.patch`).
- **Sort menu** (Settings → Appearance, off by default): Default / Newest / Oldest / Highest score / Lowest score in the search bar, in place of typing `order:` terms. Default searches exactly what is typed (`order:` terms included); your folders have no scores, so the score orders show them newest first.
- **e621 pools** (comics): search `pool:17870` or paste a pool link like `https://e621.net/pools/17870`. The pages come from e621 in the pool's own order, whatever sites are selected. **☆ Save pool** in the search bar saves it. **📚 Pools** in the search bar opens the saved pools as covers (with a filter); a click opens one, and **Find pools in favorites** adds every e621 pool your e621 favorites are in.
- **Offline copies** (Settings → Favorites): download every favorite and/or every saved pool automatically, in the background (favorites into `favorites`, pools into `pools/<name> (<number>)/001 …` in the download folder). With **Use downloaded copies**, images and videos that are on the disk are shown from there instead of the site.
- The pools window sorts by recently added, name or most pages.
- **Random** shows the favorites in a random order. The saved order doesn't change; press Filter with an empty box to go back to it.

Fixed from the extension: the favorites page now loads the saved favorites, plays videos, runs the slideshow and preloads the next slides with thumbnails. A slide that fails to load no longer stops the slideshow on either page.

## MSG-native

The same app drawn with egui instead of the web view: a simpler window with the search, slideshow, tags, favorites (import, filter, random, e621 sync), pools, quick searches and cards, downloads and offline copies, your folders, rebindable hotkeys and the settings. The site code (`src-tauri/src/native/sites.rs`) and the search state are ported from the JavaScript to Rust. It uses OpenGL (eframe's glow backend), because libmpv draws the video into it.

For testing, `MSG_SEARCH=<tags>` makes it search at start, and `MSG_SHOT=<file.png>` saves a screenshot after `MSG_SHOT_AFTER` seconds (default 12) and exits.

## Fixed keys (MSG)

- F11: fullscreen (Esc exits)
- Ctrl+R / F5: reload
- Ctrl+Shift+I / F12: developer tools

The other hotkeys can be changed in Settings → Hotkeys.

## License and credits

MSG is based on version 10.6 of [BooruSlideshow](https://github.com/Chirmaya/BooruSlideshow) by Chirmaya, whose license (see `LICENSE`) is:

> Copyright (c) 2026 Chirmaya
>
> Open license to copy/modify/etc. as long as you:
> - Don't publish under the name "Booru Slideshow" (to reduce name confusion)

That is why this app is called MSG and has its own icons. The license text and credits are also in Settings → About, along with michutsu's [BooruSlideshowElectron](https://github.com/michutsu/BooruSlideshowElectron) (the earlier Electron port; no code from it is used) [Tauri](https://tauri.app/) (MIT or Apache-2.0) with the Rust libraries under it, and [Libraries.dev](https://libraries.dev) by Jakub Antalik (MIT): the search bar's [border beam](https://libraries.dev/beam) (`css/vendor/border-beam.css`, the CSS the `border-beam` package generates) and the [thinking orb](https://libraries.dev/orbs) loading animation (`js/vendor/thinking-orbs-engine.js`, the `thinking-orbs` engine as a plain script, cut down to the searching orb, drawn by `js/loading_orb.js`). MSG-native uses [egui](https://github.com/emilk/egui) and eframe (MIT or Apache-2.0) and, for video, loads [libmpv](https://mpv.io/) (LGPL-2.1 or later) at run time; it is not included in the packages. The Refract effects style (a test) uses the glass shader of [liquid-glass-js](https://github.com/dashersw/liquid-glass-js) by Armagan Amcalar (MIT), in `js/liquid_glass.js`. Their license text is at the top of those files.

Settings and favorites saved before the rename (in `%APPDATA%/booruslideshowelectron`) are copied over on the first start.

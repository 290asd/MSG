# MSG

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/msg_icon_white_256.png">
  <img src="img/msg_icon_black_256.png" width="120" align="right" alt="MSG logo">
</picture>

A desktop slideshow for booru sites. It started as an Electron version of [Chirmaya's BooruSlideshow](https://github.com/Chirmaya/BooruSlideshow) browser extension (v10.6) and replaces the old [BooruSlideshowElectron](https://github.com/michutsu/BooruSlideshowElectron) port.

The name: MSG, monosodium glutamate, is food additive E621. The logo (`img/msg_logo.svg`, and `img/msg_logo_small.svg` for 32 px and below) is its structural formula, black on the light theme and white on the dark one.

The extension's logic (`js/`) comes from the original. `preload.js` provides the `chrome.*` API calls it uses:

| Extension API | Electron replacement |
|---|---|
| `chrome.storage.sync` / `local` | `settings.json` in the app's user data folder |
| `chrome.downloads.download` | Saved to the download folder (Settings → Folders, default `Downloads/MSG/`) |
| `rules_header_referer.json` (Gelbooru Referer) | `session.webRequest` in `main.js`, which also sets the Referer that e621's image server requires |
| host permissions | `webSecurity: false` (the page reaches booru APIs without CORS) |
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
- Effects: Glass, or Solid, which turns off the blur and the animations (the loading orb keeps turning).
- The search bar on the start page has a rainbow border beam until the first search; an image that is a favorite gets the same beam around it.
- Quick searches on keys 1–0 (Settings → Quick searches): each keeps sites and, if wanted, tags; pressing the key selects those sites and searches (for its tags, or for what is in the search box).
- U brings up the controls (the search at the top of the window); again goes back to the image. No scrollbars.
- Rule34 works again through its new API, with the user ID and API key under Sites & accounts (rule34.xxx → My Account → Options).

These live in `js/app_settings.js`. It changes hotkeys by setting the key constants in `globals.js`, which the views read on every key press.

**Favorites**

- **Import** (Settings → Favorites) adds your favorites from e621 (username), Derpibooru (API key) and Gelbooru (user ID and API key), using the details under Sites & accounts. Already-added items are skipped, so it can be run again to pick up new favorites.
- Faving or unfaving shows a heart (white when unfaved), and the card with the next slides is outlined with "♥ In favorites" while the image is a favorite. The search bar stays at the top of the window when scrolled past.
- **Also fave on e621** (Settings → Favorites, on by default): faving or unfaving an e621 image in the app does the same on your e621 account. Needs the e621 username and API key under Sites & accounts. Only failures are told.
- **Download all favorites** (Settings → Favorites) saves the favorites being shown (all, filtered or random) to `Downloads/MSG/favorites`, 3 at a time. Files already there are skipped, so a cancelled download can be resumed.
- **Clicking a tag** (Settings → Appearance → Tags) searches or filters for that tag alone, or adds it to what is in the box.
- The **site menu** next to the filter box shows favorites from one site only (with a count for each). It works together with the tag filter, Random and Download all favorites.
- **♥ joi** (test, turn on in Settings → Favorites): in the search bar and on the favorites page, opens [joi.how](https://github.com/clynamic/joi.how)'s settings over the page, with the search results loaded so far or the favorites being shown (all, filtered or random) as its images; Begin starts the game there, Esc or × closes it. The `joi` folder is a build of joi.how (GPL-3.0, see `joi/README.md`, `joi/LICENSE` and MSG's changes in `joi/msg-changes.patch`).
- **Sort menu** (Settings → Appearance, off by default): Default / Newest / Oldest / Highest score / Lowest score in the search bar, in place of typing `order:` terms. Default searches exactly what is typed (`order:` terms included); your folders have no scores, so the score orders show them newest first.
- **e621 pools** (comics): search `pool:17870` or paste a pool link like `https://e621.net/pools/17870`. The pages come from e621 in the pool's own order, whatever sites are selected. **☆ Save pool** in the search bar saves it. **📚 Pools** in the search bar opens the saved pools as covers (with a filter); a click opens one, and **Find pools in favorites** adds every e621 pool your e621 favorites are in.
- **Offline copies** (Settings → Favorites): download every favorite and/or every saved pool automatically, in the background (favorites into `favorites`, pools into `pools/<name> (<number>)/001 …` in the download folder). With **Use downloaded copies**, images and videos that are on the disk are shown from there instead of the site.
- The pools window sorts by recently added, name or most pages.
- **Random** shows the favorites in a random order. The saved order doesn't change; press Filter with an empty box to go back to it.

Fixed from the extension: the favorites page now loads the saved favorites, plays videos, runs the slideshow and preloads the next slides with thumbnails. A slide that fails to load no longer stops the slideshow on either page.

## Running

Requires [Node.js](https://nodejs.org/).

```
npm install
npm start
```

## Install

Download a package from [Releases](../../releases) and unpack it anywhere; nothing is installed.

- **Windows**: `MSG-windows-x64.zip`, then run `MSG.exe`.
- **Linux** (Ubuntu and others, x64): `MSG-linux-x64.tar.gz`, then run `./MSG`. If it stops with a message about `chrome-sandbox`, give the sandbox helper its rights once: `sudo chown root:root chrome-sandbox && sudo chmod 4755 chrome-sandbox` (or start it with `./MSG --no-sandbox`).
- **macOS**: `MSG-macos-arm64.zip` (Apple Silicon) or `MSG-macos-x64.zip` (Intel). The app is not signed, so the first time right-click MSG.app → Open, or run `xattr -cr MSG.app`.

Settings and favorites are kept in the user data folder: `%APPDATA%\MSG` on Windows, `~/.config/MSG` on Linux and `~/Library/Application Support/MSG` on macOS.

## Building

Portable apps go into `MSG-<platform>-<arch>/`:

```
npm run build        # Windows
npm run build:linux  # Linux x64
npm run build:mac    # macOS x64 and arm64 (needs macOS, Linux, or Windows as admin, for the app bundle's symlinks)
```

Pushing a tag like `v1.0.0` runs `.github/workflows/build.yml`, which builds all three on GitHub and attaches them to that release.

## Fixed keys

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

That is why this app is called MSG and has its own icons. The license text and credits are also in Settings → About, along with michutsu's [BooruSlideshowElectron](https://github.com/michutsu/BooruSlideshowElectron) (the earlier Electron port; no code from it is used) [Electron](https://www.electronjs.org/) (MIT; a build includes Electron's `LICENSE` and `LICENSES.chromium.html`), and [Libraries.dev](https://libraries.dev) by Jakub Antalik (MIT): the search bar's [border beam](https://libraries.dev/beam) (`css/vendor/border-beam.css`, the CSS the `border-beam` package generates) and the [thinking orb](https://libraries.dev/orbs) loading animation (`js/vendor/thinking-orbs-engine.js`, the `thinking-orbs` engine as a plain script, drawn by `js/loading_orb.js`). The Refract effects style (a test) uses the glass shader of [liquid-glass-js](https://github.com/dashersw/liquid-glass-js) by Armagan Amcalar (MIT), in `js/liquid_glass.js`. Their license text is at the top of those files.

Settings and favorites saved before the rename (in `%APPDATA%/booruslideshowelectron`) are copied over on the first start.

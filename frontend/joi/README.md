# joi.how in MSG

This folder is a build of [joi.how](https://github.com/clynamic/joi.how) by clynamic,
licensed under the GNU General Public License v3.0 (see `LICENSE` in this folder).

- Source: https://github.com/clynamic/joi.how, commit 86b0468ea83ff55e6cfda14f56495bd25aa5a766
- MSG's changes to it: `msg-changes.patch` (apply with `git apply` on that commit)
- Built with `yarn install --frozen-lockfile` and `npx vite build`; `dist/` minus the website-only files
  (service worker, manifest, icons, banner) is what is here.

The changes make it run from a file inside MSG: hash routes instead of browser routes, relative
paths, no service worker, "Leave" after the climax goes back to the settings, and the home page shows
only the settings and Begin (no age confirmation), because the images are the favorites MSG passes in
(localStorage "images"). The game's settings dialog has no image grid (it froze with hundreds of
images), the game's top bar leaves room for MSG's close button, and `src/msg-theme.css` and
`src/msg.ts` give it MSG's glass look, follow MSG's light/dark theme and close it with Esc.

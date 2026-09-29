// window.chrome for the extension's code, made from what preload.js exposes (the preload runs
// in its own world, so it can't set window.chrome itself). Loaded before every other script.
window.chrome = Object.assign(window.chrome || {}, { runtime: { lastError: undefined } }, window.msgChrome);

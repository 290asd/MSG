// Applies the saved theme before the page is drawn, so it doesn't flash in the wrong theme.
// The choice itself is stored by app_settings.js; this copy in localStorage is only for speed.
(function () {
    let theme = 'dark';

    try {
        theme = localStorage.getItem('appTheme') || 'dark';
    } catch (e) {}

    if (theme == 'system')
        theme = matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';

    document.documentElement.dataset.theme = theme;
})();

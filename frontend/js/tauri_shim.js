// The native side (src-tauri) behind the same window.msgChrome / window.appInfo the pages were written for,
// so the pages, written for the browser extension, run unchanged.
(function () {
    const { invoke, convertFileSrc } = window.__TAURI__.core;
    const { listen } = window.__TAURI__.event;

    const withCallback = (promise, callback) => {
        if (typeof callback === 'function') promise.then(callback);
        return promise;
    };

    const storageArea = {
        get: (keys, callback) => withCallback(invoke('storage_get', { keys: keys === undefined ? null : keys }), callback),
        set: (items, callback) => withCallback(invoke('storage_set', { items }), callback),
        remove: (keys, callback) => withCallback(invoke('storage_remove', { keys }), callback)
    };

    // A file on this computer, as a URL the page may load.
    const fileUrl = path => convertFileSrc(path);

    // ---------- The window ----------
    const assetPrefix = convertFileSrc('');
    window.isLocalFileUrl = url => typeof url == 'string' && (/^file:/i.test(url) || url.startsWith(assetPrefix));
    // A Windows path keeps its backslashes: the asset scope compares paths as written.
    const pathOfFileUrl = url => {
        const path = /^file:/i.test(url)
            ? decodeURIComponent(new URL(url).pathname).replace(/^\/([A-Za-z]:)/, '$1')
            : decodeURIComponent(url.slice(assetPrefix.length).split(/[?#]/)[0]);
        return /^[A-Za-z]:/.test(path) ? path.replace(/\//g, '\\') : path;
    };

    // window.open and links go to the system browser; a file from your own folders is shown in Explorer.
    window.open = url => {
        url = String(url);
        if (window.isLocalFileUrl(url)) invoke('show_in_folder', { path: pathOfFileUrl(url) });
        else if (/^https?:/i.test(url)) invoke('open_external', { url });
        return null;
    };
    document.addEventListener('click', event => {
        const link = event.target.closest && event.target.closest('a[href]');
        if (link && /^https?:/i.test(link.href) && !link.href.startsWith(location.origin + '/')) {
            event.preventDefault();
            invoke('open_external', { url: link.href });
        }
    });

    document.addEventListener('keydown', event => {
        const ctrl = event.ctrlKey || event.metaKey;
        if (event.key == 'F11') invoke('toggle_fullscreen');
        else if (event.key == 'F12' || (ctrl && event.shiftKey && event.key.toLowerCase() == 'i')) invoke('toggle_devtools');
        else if (event.key == 'F5' || (ctrl && !event.shiftKey && event.key.toLowerCase() == 'r')) location.reload();
        else if (event.key == 'Escape') return invoke('exit_fullscreen');
        else return;
        event.preventDefault();
    });

    // ---------- The internet ----------
    // The page's origin can't call the booru sites (CORS), so XHR and fetch to them go through Rust,
    // and remote images and videos load through the msg-proxy scheme (which adds each site's Referer).
    const proxyPrefix = convertFileSrc('', 'msg-proxy');
    // Ours already (the proxy, local files, the app's own pages) stays as it is.
    const isOurs = url => [proxyPrefix, assetPrefix, location.origin + '/', 'http://ipc.localhost'].some(prefix => url.startsWith(prefix));
    // A file:// URL is one saved by an earlier version (the background image): it becomes an asset URL.
    const fromFileUrl = url => convertFileSrc(pathOfFileUrl(url));
    window.msgProxyUrl = url => {
        if (typeof url != 'string') return url;
        if (/^file:/i.test(url)) return fromFileUrl(url);
        return /^https?:/i.test(url) && !isOurs(url) ? convertFileSrc(url, 'msg-proxy') : url;
    };

    const rustRequest = (url, options) => invoke('http_request', {
        url, method: options.method, headers: options.headers, body: options.body, timeoutMs: options.timeout
    });

    // Only what web_requester.js uses.
    window.XMLHttpRequest = class {
        constructor() {
            this.timeout = 0;
            this.status = 0;
            this.responseText = '';
            this.headers = {};
            this.aborted = false;
        }

        open(method, url) {
            this.method = method;
            this.url = url;
        }

        setRequestHeader(name, value) {
            this.headers[name] = value;
        }

        abort() {
            this.aborted = true;
        }

        send(body) {
            rustRequest(this.url, { method: this.method, headers: this.headers, body: body == null ? undefined : String(body), timeout: this.timeout })
                .then(response => {
                    if (this.aborted) return;
                    this.status = response.status;
                    this.responseText = response.body;
                    if (this.onload) this.onload();
                }, error => {
                    if (this.aborted) return;
                    const handler = error == 'timeout' ? this.ontimeout : this.onerror;
                    if (handler) handler();
                });
        }
    };

    const pageFetch = window.fetch.bind(window);
    window.fetch = async (input, init = {}) => {
        const url = typeof input == 'string' ? input : input.url;
        if (!/^https?:/i.test(url) || isOurs(url)) return pageFetch(input, init); // includes Tauri's own IPC calls
        const method = (init.method || 'GET').toUpperCase();
        try {
            const response = await rustRequest(url, { method, headers: Object.fromEntries(new Headers(init.headers || {})), body: init.body });
            return new Response(method == 'HEAD' || [204, 205, 304].includes(response.status) ? null : response.body, { status: response.status, headers: response.headers });
        } catch (error) {
            throw new TypeError('Failed to fetch (' + error + ')'); // never the address: it can hold an API key
        }
    };

    // Every image and video src goes through the proxy; the elements are made CORS-enabled so
    // Refract (liquid_glass.js) can still draw them on a canvas.
    for (const element of [HTMLImageElement, HTMLMediaElement, HTMLSourceElement]) {
        const descriptor = Object.getOwnPropertyDescriptor(element.prototype, 'src');
        Object.defineProperty(element.prototype, 'src', {
            ...descriptor,
            set(value) {
                const proxied = window.msgProxyUrl(String(value));
                if ('crossOrigin' in this && (proxied.startsWith(proxyPrefix) || proxied.startsWith(assetPrefix))) this.crossOrigin = 'anonymous';
                descriptor.set.call(this, proxied);
            }
        });
    }

    window.appInfo = {
        getVersion: () => invoke('app_version'),
        chooseBackground: () => invoke('choose_background').then(path => path && fileUrl(path)),
        clearBackground: () => invoke('clear_background'),
        backgroundFromUrl: url => invoke('background_from_url', { url }).then(fileUrl),
        randomFavoriteImage: (gifs, videos) => invoke('random_favorite_image', { gifs: !!gifs, videos: !!videos }).then(path => path && fileUrl(path)),
        chooseFolder: title => invoke('choose_folder', { title }),
        listLocalMedia: folders => invoke('list_local_media', { folders }).then(files => files.map(f => ({ ...f, url: fileUrl(f.path_abs) }))),
        listLocalCopies: () => invoke('list_local_copies').then(copies => {
            for (const name in copies) copies[name] = fileUrl(copies[name]);
            return copies;
        }),
        offlineFolders: () => invoke('offline_folders'),
        autoDownloadStatus: () => invoke('auto_download_status'),
        onAutoDownloadStatus: callback => listen('auto-download-status', e => callback(e.payload)),
        onDownloadProgress: callback => listen('download-progress', e => callback(e.payload)),
        onLocalCopiesChanged: callback => listen('local-copies-changed', () => callback()),
        themeChanged: dark => invoke('theme_changed', { dark }),
        // Shown in Settings → About and Developer.
        runtime: 'Tauri'
    };
    const webView = (navigator.userAgent.match(/Edg\/([\d.]+)/) || navigator.userAgent.match(/Version\/([\d.]+)/) || [])[1];
    window.__TAURI__.app.getTauriVersion().then(version => window.appInfo.runtime = 'Tauri ' + version + (webView ? ' · web view ' + webView : ''));

    window.msgChrome = {
        storage: { sync: storageArea, local: storageArea },
        downloads: {
            download: (options, callback) => withCallback(invoke('download', { url: options.url }), callback),

            // Not part of chrome.*: downloads many files into <download folder>/<folder>, reporting progress.
            downloadMany: async (urls, folder, onProgress) => {
                const unlisten = await listen('download-many-progress', e => onProgress(e.payload));
                try {
                    return await invoke('download_many', { urls, folder });
                } finally {
                    unlisten();
                }
            },
            cancelDownloadMany: () => invoke('download_many_cancel')
        }
    };
})();

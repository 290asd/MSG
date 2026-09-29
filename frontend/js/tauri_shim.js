// The native side (src-tauri) behind the same window.msgChrome / window.appInfo the pages were written for,
// so the rest of the code doesn't know it isn't running in Electron any more.
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
        // Shown in Settings → About.
        runtimeName: 'Tauri',
        runtimeVersion: window.__TAURI__.app ? '' : ''
    };

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

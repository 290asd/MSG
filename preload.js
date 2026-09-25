// Provides the small part of the chrome.* extension API that BooruSlideshow uses,
// so the extension's files run unchanged inside Electron.
// The page runs in its own world (contextIsolation), so only these functions reach it.
const { contextBridge, ipcRenderer } = require('electron')

function withCallback (promise, callback) {
  if (typeof callback === 'function') promise.then(callback)
  return promise
}

const storageArea = {
  get: (keys, callback) => withCallback(ipcRenderer.invoke('storage-get', keys), callback),
  set: (items, callback) => withCallback(ipcRenderer.invoke('storage-set', items), callback),
  remove: (keys, callback) => withCallback(ipcRenderer.invoke('storage-remove', keys), callback)
}

// For the About section in the settings.
contextBridge.exposeInMainWorld('appInfo', {
  getVersion: () => ipcRenderer.invoke('app-version'),
  // Settings → Appearance: returns the file:// URL of the chosen image, or null if cancelled.
  chooseBackground: () => ipcRenderer.invoke('choose-background'),
  clearBackground: () => ipcRenderer.invoke('clear-background'),
  backgroundFromUrl: (url) => ipcRenderer.invoke('background-from-url', url),
  randomFavoriteImage: (gifs, videos) => ipcRenderer.invoke('random-favorite-image', gifs, videos),
  // Settings → Folders: a folder picker, and the media files in the chosen folders.
  chooseFolder: (title) => ipcRenderer.invoke('choose-folder', title),
  listLocalMedia: (folders) => ipcRenderer.invoke('list-local-media', folders),
  // Offline copies: {site file name: file:// URL}, and the background download's progress.
  listLocalCopies: () => ipcRenderer.invoke('list-local-copies'),
  offlineFolders: () => ipcRenderer.invoke('offline-folders'),
  autoDownloadStatus: () => ipcRenderer.invoke('auto-download-status'),
  onAutoDownloadStatus: (callback) => ipcRenderer.on('auto-download-status', (event, text) => callback(text)),
  onLocalCopiesChanged: (callback) => ipcRenderer.on('local-copies-changed', () => callback()),
  // Switches the window icon between the white (dark theme) and black logo.
  themeChanged: (dark) => ipcRenderer.send('theme-changed', dark),
  electronVersion: process.versions.electron,
  chromeVersion: process.versions.chrome
})

// js/chrome_api.js makes this window.chrome for the extension's code.
contextBridge.exposeInMainWorld('msgChrome', {
  storage: { sync: storageArea, local: storageArea },
  downloads: {
    download: (options, callback) => withCallback(ipcRenderer.invoke('download', {
      url: options.url,
      filename: options.filename
    }), callback),

    // Not part of chrome.*: downloads many files into Downloads/<folder>, reporting progress.
    downloadMany: async (urls, folder, onProgress) => {
      const listener = (event, progress) => onProgress(progress)
      ipcRenderer.on('download-many-progress', listener)
      try {
        return await ipcRenderer.invoke('download-many', { urls, folder })
      } finally {
        ipcRenderer.removeListener('download-many-progress', listener)
      }
    },
    cancelDownloadMany: () => ipcRenderer.invoke('download-many-cancel')
  }
})

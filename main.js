const { app, BrowserWindow, dialog, ipcMain, nativeTheme, net, session, shell } = require('electron')
const path = require('path')
const fs = require('fs')
const { pathToFileURL, fileURLToPath } = require('url')

// Everything chrome.storage.sync/local held in the extension lives in one JSON file.
const storeFile = path.join(app.getPath('userData'), 'settings.json')
let store = {}

// Before the rename to MSG, and in the old BooruSlideshowElectron, the data lived in
// <AppData>/booruslideshowelectron.
const legacyUserData = path.join(app.getPath('appData'), 'booruslideshowelectron')

function loadStore () {
  try {
    store = JSON.parse(fs.readFileSync(storeFile, 'utf8'))
  } catch (e) {
    store = readLegacyStore() || importOldElectronSettings()
    saveStore()
  }
}

// The file holds the favorites too (several MB), so writes are grouped: at most one every 300 ms,
// off the main thread. They go to a temporary file first, so a crash mid-write can't corrupt it.
let saveTimer = null
let saving = Promise.resolve()

function saveStore () {
  if (!saveTimer) saveTimer = setTimeout(writeStore, 300)
}

function writeStore () {
  saveTimer = null
  const data = JSON.stringify(store)
  saving = saving.then(async () => {
    await fs.promises.mkdir(path.dirname(storeFile), { recursive: true })
    await fs.promises.writeFile(storeFile + '.tmp', data)
    await fs.promises.rename(storeFile + '.tmp', storeFile)
  }).catch(e => console.error('Saving the settings failed:', e))
}

// On quit, a pending write is done right away.
function flushStore () {
  if (!saveTimer) return
  clearTimeout(saveTimer)
  saveTimer = null
  fs.mkdirSync(path.dirname(storeFile), { recursive: true })
  fs.writeFileSync(storeFile + '.tmp', JSON.stringify(store))
  fs.renameSync(storeFile + '.tmp', storeFile)
}

// Settings and favorites saved under the app's earlier name.
function readLegacyStore () {
  try {
    return JSON.parse(fs.readFileSync(path.join(legacyUserData, 'settings.json'), 'utf8'))
  } catch (e) {
    return null
  }
}

// The old BooruSlideshowElectron used electron-json-storage: <userData>/storage/<key>.json = {key: value}
function importOldElectronSettings () {
  const result = {}
  const dir = path.join(legacyUserData, 'storage')
  if (!fs.existsSync(dir)) return result
  for (const file of fs.readdirSync(dir)) {
    if (!file.endsWith('.json')) continue
    const key = path.basename(file, '.json')
    try {
      const data = JSON.parse(fs.readFileSync(path.join(dir, file), 'utf8'))
      if (data && Object.prototype.hasOwnProperty.call(data, key)) result[key] = data[key]
    } catch (e) {}
  }
  return result
}

ipcMain.handle('app-version', () => app.getVersion())

// The logo is white on the dark theme and black on the light one.
function windowIcon (dark) {
  return path.join(__dirname, 'img', dark ? 'msg_icon_white_256.png' : 'msg_icon_black_256.png')
}

// The theme saved in the settings (app_settings.js), before the page has told us.
function isDarkTheme () {
  const theme = store.appTheme || 'dark'
  return theme === 'system' ? nativeTheme.shouldUseDarkColors : theme === 'dark'
}

ipcMain.on('theme-changed', (event, dark) => {
  const win = BrowserWindow.fromWebContents(event.sender)
  if (win) win.setIcon(windowIcon(dark))
})

// Background image (Settings → Appearance): a copy is kept in the user data folder,
// under a new name each time so the page doesn't show a cached old one.
function removeBackgroundImages () {
  for (const file of fs.readdirSync(app.getPath('userData'))) {
    if (file.startsWith('background-')) fs.rmSync(path.join(app.getPath('userData'), file), { force: true })
  }
}

ipcMain.handle('choose-background', async (event) => {
  const result = await dialog.showOpenDialog(BrowserWindow.fromWebContents(event.sender), {
    title: 'Choose a background image or video',
    filters: [{ name: 'Images and videos', extensions: ['jpg', 'jpeg', 'png', 'webp', 'gif', 'bmp', 'avif', 'webm', 'mp4'] }],
    properties: ['openFile']
  })
  if (result.canceled || result.filePaths.length === 0) return null

  removeBackgroundImages()
  const target = path.join(app.getPath('userData'), 'background-' + Date.now() + path.extname(result.filePaths[0]).toLowerCase())
  fs.copyFileSync(result.filePaths[0], target)
  return pathToFileURL(target).href
})

ipcMain.handle('clear-background', () => removeBackgroundImages())

// Settings → Folders: pick a folder (to browse, or to download into).
ipcMain.handle('choose-folder', async (event, title) => {
  const result = await dialog.showOpenDialog(BrowserWindow.fromWebContents(event.sender), {
    title: title,
    properties: ['openDirectory']
  })
  return result.canceled || result.filePaths.length === 0 ? null : result.filePaths[0]
})

// The "Your folders" site: the images and videos in the chosen folders and their subfolders.
// `path` starts with the chosen folder's own name, so folder names can be searched and shown as tags.
const MEDIA_EXTENSIONS = new Set(['.jpg', '.jpeg', '.png', '.gif', '.webp', '.bmp', '.avif', '.webm', '.mp4', '.m4v', '.mov', '.ogv'])
const MAX_LOCAL_FILES = 50000 // per folder, so a big one doesn't crowd out the others

ipcMain.handle('list-local-media', (event, folders) => {
  const files = []

  let start = 0
  const walk = (dir, root) => {
    let entries
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true })
    } catch (e) {
      return // unreadable or removed folder
    }
    for (const entry of entries) {
      if (files.length - start >= MAX_LOCAL_FILES || entry.name.startsWith('.')) continue
      const full = path.join(dir, entry.name)
      if (entry.isDirectory()) {
        walk(full, root)
      } else if (MEDIA_EXTENSIONS.has(path.extname(entry.name).toLowerCase())) {
        try {
          files.push({
            url: pathToFileURL(full).href,
            path: path.relative(path.dirname(root), full),
            modified: fs.statSync(full).mtimeMs
          })
        } catch (e) {}
      }
    }
  }

  for (const folder of folders) {
    start = files.length
    walk(folder, folder)
  }
  return files
})

// Ctrl+L: the image being shown becomes the background image.
const IMAGE_EXTENSIONS = new Set(['.jpg', '.jpeg', '.png', '.webp', '.gif', '.bmp', '.avif'])
const VIDEO_EXTENSIONS = new Set(['.webm', '.mp4'])

ipcMain.handle('background-from-url', async (event, url) => {
  if (!/^https?:/i.test(url)) throw new Error('Not a web image')
  const referer = refererFor(url)
  const response = await net.fetch(url, { headers: referer ? { Referer: referer } : {} })
  if (!response.ok) throw new Error('HTTP ' + response.status)
  const data = Buffer.from(await response.arrayBuffer())

  removeBackgroundImages()
  const urlExt = path.extname(new URL(url).pathname).toLowerCase()
  const ext = IMAGE_EXTENSIONS.has(urlExt) || VIDEO_EXTENSIONS.has(urlExt) ? urlExt : '.jpg'
  const target = path.join(app.getPath('userData'), 'background-' + Date.now() + ext)
  fs.writeFileSync(target, data)
  return pathToFileURL(target).href
})

// Settings → Appearance: a random image among the downloaded favorites, as the background at start.
// Still images always; GIFs and videos (.webm, .mp4) only when asked for.
ipcMain.handle('random-favorite-image', (event, gifs, videos) => {
  const dir = downloadPath('MSG/favorites')
  const allowed = new Set([...IMAGE_EXTENSIONS].filter(ext => ext !== '.gif'))
  if (gifs) allowed.add('.gif')
  if (videos) VIDEO_EXTENSIONS.forEach(ext => allowed.add(ext))
  let images
  try {
    images = fs.readdirSync(dir).filter(file => allowed.has(path.extname(file).toLowerCase()))
  } catch (e) {
    return null // no favorites folder yet
  }
  return images.length ? pathToFileURL(path.join(dir, images[Math.floor(Math.random() * images.length)])).href : null
})

ipcMain.handle('storage-get', (event, keys) => {
  if (keys == null) return { ...store }
  if (typeof keys === 'string') keys = [keys]
  const result = {}
  if (Array.isArray(keys)) {
    for (const key of keys) if (key in store) result[key] = store[key]
  } else {
    for (const key in keys) result[key] = key in store ? store[key] : keys[key]
  }
  return result
})

ipcMain.handle('storage-set', (event, items) => {
  Object.assign(store, items)
  saveStore()
  if (['personalListItems', 'savedPools', 'autoDownloadFavorites', 'autoDownloadPools', 'downloadFolder', 'offlineMode'].some(key => key in items)) scheduleAutoDownload()
})

ipcMain.handle('storage-remove', (event, keys) => {
  for (const key of [].concat(keys)) delete store[key]
  saveStore()
})

// Replacement for chrome.downloads.download: saves to downloadPath(<filename>), overwriting.
const pendingDownloads = new Map()

// Where downloads go: the folder chosen in Settings → Folders, or <Downloads>/MSG.
// The pages ask for paths like "MSG/<file>" and "MSG/favorites"; the "MSG/" part means that folder.
// A name that tries to leave the folder ("..\\", an absolute path) is refused.
function downloadPath (relative) {
  // Only a full path counts; anything else (a broken setting) falls back to Downloads/MSG.
  const chosen = typeof store.downloadFolder === 'string' && path.isAbsolute(store.downloadFolder) ? store.downloadFolder : null
  const base = path.resolve(chosen || path.join(app.getPath('downloads'), 'MSG'))
  const target = path.resolve(base, String(relative).replace(/^MSG[\/]?/, ''))
  if (target !== base && !target.startsWith(base + path.sep)) throw new Error('Download outside the download folder: ' + relative)
  fs.mkdirSync(path.dirname(target), { recursive: true })
  return target
}

// A file name from a URL, safe for Windows: no folders, no reserved characters.
function safeFileName (url) {
  let name
  try {
    name = decodeURIComponent(new URL(url).pathname.split('/').pop())
  } catch (e) {
    name = ''
  }
  return cleanFileName(name) || 'file-' + Date.now()
}

// No folders, no characters Windows doesn't allow, no leading dots.
function cleanFileName (name) {
  return name.replace(/[<>:"/\\|?*\x00-\x1f]/g, '_').replace(/^\.+/, '').trim().slice(0, 200)
}

ipcMain.handle('download', (event, options) => {
  if (!/^https?:/i.test(options.url)) return
  // Downloads made with the button or L go to <folder>/downloads; favorites have their own folder.
  pendingDownloads.set(options.url, 'downloads/' + safeFileName(options.url))
  event.sender.downloadURL(options.url)
})

// Bulk download (Download all favorites in the favorites page's settings): 3 files at a time into
// downloadPath(<folder>), skipping files that already exist so a rerun resumes.
let bulkDownloadCancelled = false

// The Referer each site's image server expects.
function refererFor (url) {
  const host = new URL(url).hostname
  if (host.endsWith('e621.net')) return 'https://e621.net/'
  if (host.endsWith('gelbooru.com')) return 'https://gelbooru.com'
  if (host.endsWith('rule34.xxx')) return 'https://rule34.xxx/'
  return undefined
}

async function downloadFile (url, filePath) {
  if (fs.existsSync(filePath) && fs.statSync(filePath).size > 0) return 'skipped'
  const referer = refererFor(url)
  const response = await net.fetch(url, { headers: referer ? { Referer: referer } : {} })
  if (!response.ok) throw new Error('HTTP ' + response.status)
  const partPath = filePath + '.part'
  fs.writeFileSync(partPath, Buffer.from(await response.arrayBuffer()))
  fs.renameSync(partPath, filePath)
  return 'downloaded'
}

ipcMain.handle('download-many', async (event, { urls, folder }) => {
  bulkDownloadCancelled = false
  const dir = downloadPath(folder)
  fs.mkdirSync(dir, { recursive: true })

  const result = { total: urls.length, downloaded: 0, skipped: 0, failed: 0, cancelled: false, folder: dir }
  let next = 0

  async function worker () {
    while (next < urls.length && !bulkDownloadCancelled) {
      const url = urls[next++]
      try {
        if (!/^https?:/i.test(url)) throw new Error('Not a web file')
        result[await downloadFile(url, path.join(dir, safeFileName(url)))]++
      } catch (e) {
        result.failed++
      }
      if (!event.sender.isDestroyed()) event.sender.send('download-many-progress', result)
    }
  }

  await Promise.all([worker(), worker(), worker()])
  result.cancelled = bulkDownloadCancelled
  return result
})

ipcMain.handle('download-many-cancel', () => {
  bulkDownloadCancelled = true
})

// ---------- Offline copies (Settings → Favorites) ----------
// Favorites go to <download folder>/favorites/<file name>, saved pools to
// <download folder>/pools/<name> (<id>)/<page number> <file name>. This runs in the background here, so it
// goes on while the pages change. Files already there are skipped.

const autoDownload = { running: false, again: false, timer: null, status: '' }

function scheduleAutoDownload (delay = 5000) {
  clearTimeout(autoDownload.timer)
  autoDownload.timer = setTimeout(runAutoDownload, delay)
}

function autoDownloadStatus (text) {
  autoDownload.status = text
  for (const win of BrowserWindow.getAllWindows()) win.webContents.send('auto-download-status', text)
}

// e621's API from here: its rate limit wants a pause between requests and a descriptive User-Agent.
async function e621Json (pathAndQuery) {
  await new Promise(resolve => setTimeout(resolve, 700))
  const headers = { 'User-Agent': 'MSG/' + app.getVersion() + ' (offline copies)' }
  if (store.e621Login && store.e621ApiKey) headers.Authorization = 'Basic ' + Buffer.from(store.e621Login + ':' + store.e621ApiKey).toString('base64')
  const response = await net.fetch('https://e621.net' + pathAndQuery, { headers })
  if (!response.ok) throw new Error('e621 answered ' + response.status)
  return response.json()
}

async function downloadAll (jobs, label) {
  let done = 0
  let failed = 0
  let next = 0
  const worker = async () => {
    while (next < jobs.length && (store.autoDownloadFavorites || store.autoDownloadPools) && !store.offlineMode) {
      const job = jobs[next++]
      try {
        if (await downloadFile(job.url, job.path) === 'downloaded') done++
      } catch (e) {
        failed++
      }
      if (next % 20 === 0) autoDownloadStatus(label + ': ' + next + ' / ' + jobs.length)
    }
  }
  await Promise.all([worker(), worker(), worker()])
  // Stopped: the setting was turned off before every file was tried.
  return { done, failed, stopped: next < jobs.length }
}

async function runAutoDownload () {
  if (store.offlineMode) return autoDownloadStatus('Offline mode is on, so nothing is downloaded.')
  if (autoDownload.running) {
    autoDownload.again = true
    return
  }
  autoDownload.running = true
  let newFiles = 0

  try {
    if (store.autoDownloadFavorites) {
      const dir = downloadPath('MSG/favorites')
      fs.mkdirSync(dir, { recursive: true })
      const jobs = (store.personalListItems || [])
        .filter(item => item && /^https?:/i.test(item.fileUrl))
        .map(item => ({ url: item.fileUrl, path: path.join(dir, safeFileName(item.fileUrl)) }))
        .filter(job => !fs.existsSync(job.path))
      if (jobs.length) newFiles += (await downloadAll(jobs, 'Favorites')).done
    }

    if (store.autoDownloadPools) {
      const finished = store.downloadedPools || {}
      for (const pool of store.savedPools || []) {
        if (!store.autoDownloadPools) break
        if (finished[pool.id] >= pool.count) continue

        const info = await e621Json('/pools/' + Number(pool.id) + '.json')
        const ids = info.post_ids || []
        const urls = new Map()
        for (let i = 0; i < ids.length; i += 100) {
          const data = await e621Json('/posts.json?limit=100&tags=id:' + ids.slice(i, i + 100).join(','))
          for (const post of data.posts) if (post.file && post.file.url) urls.set(post.id, post.file.url)
        }

        const dir = downloadPath('MSG/pools/' + cleanFileName(String(info.name).replace(/_/g, ' ') + ' (' + info.id + ')'))
        fs.mkdirSync(dir, { recursive: true })
        const digits = String(ids.length).length
        const jobs = ids.map((id, i) => ({ url: urls.get(id), page: i + 1 })).filter(job => job.url).map(job => ({
          url: job.url,
          path: path.join(dir, String(job.page).padStart(Math.max(3, digits), '0') + ' ' + safeFileName(job.url))
        })).filter(job => !fs.existsSync(job.path))

        const result = jobs.length ? await downloadAll(jobs, info.name.replace(/_/g, ' ')) : { done: 0, failed: 0, stopped: false }
        newFiles += result.done
        // Done when every page that has a file is here; a pool that grows later is checked again.
        if (result.failed === 0 && !result.stopped) {
          store.downloadedPools = Object.assign({}, store.downloadedPools, { [pool.id]: ids.length })
          saveStore()
        }
      }
    }
    autoDownloadStatus(newFiles ? 'Downloaded ' + newFiles + ' new files.' : 'Everything is downloaded.')
  } catch (e) {
    autoDownloadStatus('Stopped: ' + e.message + '. Tries again later.')
    scheduleAutoDownload(10 * 60 * 1000)
  } finally {
    autoDownload.running = false
    if (newFiles) for (const win of BrowserWindow.getAllWindows()) win.webContents.send('local-copies-changed')
    if (autoDownload.again) {
      autoDownload.again = false
      scheduleAutoDownload(1000)
    }
  }
}

ipcMain.handle('auto-download-status', () => autoDownload.status)

// Offline mode: the downloaded favorites and pools are browsed like your own folders.
ipcMain.handle('offline-folders', () => ['MSG/favorites', 'MSG/pools'].map(folder => downloadPath(folder)).filter(dir => fs.existsSync(dir)))

// Downloaded copies, by the name of the file on the site: pool pages have their page number in front.
ipcMain.handle('list-local-copies', () => {
  const copies = {}
  const base = downloadPath('MSG/')
  const add = (dir) => {
    let entries
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true })
    } catch (e) {
      return
    }
    for (const entry of entries) {
      const full = path.join(dir, entry.name)
      if (entry.isDirectory()) add(full)
      else if (MEDIA_EXTENSIONS.has(path.extname(entry.name).toLowerCase())) copies[entry.name.replace(/^\d+ /, '')] = pathToFileURL(full).href
    }
  }
  add(path.join(base, 'favorites'))
  add(path.join(base, 'pools'))
  return copies
})

const appPagesUrl = pathToFileURL(__dirname).href + '/'

function createWindow () {
  const win = new BrowserWindow({
    width: 1280,
    height: 900,
    icon: windowIcon(isDarkTheme()),
    backgroundColor: '#0b0c10',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      // The preload gives the page window.chrome and window.appInfo through contextBridge,
      // so page scripts (and anything injected into them) can't reach Electron itself.
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
      // The extension had host permissions for every booru; this lets the
      // page's XHRs reach their APIs without CORS blocking them.
      webSecurity: false
    }
  })
  win.removeMenu()

  // Links and window.open go to the system browser.
  win.webContents.setWindowOpenHandler(({ url }) => {
    if (/^https?:/i.test(url)) shell.openExternal(url)
    // A file from your own folders ("open post", E): show it in Explorer.
    else if (url.startsWith('file:')) shell.showItemInFolder(fileURLToPath(url))
    return { action: 'deny' }
  })

  // The window only shows the app's own pages; a link that would replace them opens in the browser.
  win.webContents.on('will-navigate', (event, url) => {
    if (url.startsWith(appPagesUrl)) return
    event.preventDefault()
    if (/^https?:/i.test(url)) shell.openExternal(url)
  })

  win.webContents.on('before-input-event', (event, input) => {
    if (input.type !== 'keyDown') return
    const ctrl = input.control || input.meta
    if (input.key === 'F11') {
      win.setFullScreen(!win.isFullScreen())
    } else if (input.key === 'F12' || (ctrl && input.shift && input.key.toLowerCase() === 'i')) {
      win.webContents.toggleDevTools()
    } else if (input.key === 'F5' || (ctrl && input.key.toLowerCase() === 'r')) {
      win.reload()
    } else if (input.key === 'Escape' && win.isFullScreen()) {
      win.setFullScreen(false)
    } else {
      return
    }
    event.preventDefault()
  })

  win.loadFile('slideshow.html')
}

app.whenReady().then(() => {
  loadStore()

  // Only fullscreen (for videos); no camera, microphone, location, notifications and so on.
  session.defaultSession.setPermissionRequestHandler((webContents, permission, callback) => callback(permission === 'fullscreen'))

  // Like the extension's rules_header_referer.json: these sites reject images without their own Referer.
  // (e621's CDN returns 403 to image requests with no Referer, which file:// pages never send.)
  session.defaultSession.webRequest.onBeforeSendHeaders(
    { urls: ['https://*.gelbooru.com/*', 'https://*.e621.net/*', 'https://*.rule34.xxx/*'] },
    (details, callback) => {
      details.requestHeaders.Referer = refererFor(details.url)
      callback({ requestHeaders: details.requestHeaders })
    }
  )

  // Offline mode (Settings → Data usage): nothing from the internet, only files on this computer.
  session.defaultSession.webRequest.onBeforeRequest({ urls: ['http://*/*', 'https://*/*'] }, (details, callback) => {
    callback({ cancel: !!store.offlineMode })
  })

  session.defaultSession.on('will-download', (event, item) => {
    const url = item.getURLChain()[0]
    const filename = pendingDownloads.get(url) || 'downloads/' + (cleanFileName(item.getFilename()) || 'file-' + Date.now())
    pendingDownloads.delete(url)
    try {
      item.setSavePath(downloadPath(filename))
    } catch (e) {
      item.cancel()
      return
    }

    // Progress for the corner box (Settings → Developer): its own channel, so it doesn't
    // overwrite the background download's status.
    const name = path.basename(filename)
    const report = text => { for (const win of BrowserWindow.getAllWindows()) win.webContents.send('download-progress', text) }
    item.on('updated', () => {
      const total = item.getTotalBytes()
      report('Downloading ' + name + (total > 0 ? ': ' + Math.floor(item.getReceivedBytes() / total * 100) + '%' : ''))
    })
    item.once('done', (e, state) => report(state === 'completed' ? 'Saved ' + name : 'Download failed: ' + name))
  })

  createWindow()
  scheduleAutoDownload(15000)
})

app.on('will-quit', flushStore)

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) createWindow()
})

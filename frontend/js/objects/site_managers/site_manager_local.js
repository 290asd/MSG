// "Your folders": the images and videos in the folders chosen in Settings → Folders
// (listed by the main process, see list-local-media in main.js).
// Search words filter by folder and file names (-word leaves those out); an empty search shows everything.
class SiteManagerLocal extends SiteManager
{
    constructor(sitesManager, pageLimit)
    {
        super(sitesManager, SITE_LOCAL, 'local', pageLimit);
    }

    // Always there; nothing to ask a server.
    pingStatus(callback)
    {
        this.isOnline = true;
        callback(this);
    }

    async performSearch(searchText, doneSearchingSiteCallback)
    {
        this.ranIntoErrorWhileSearching = false;

        // Folders unticked in Settings → Folders are left out.
        let stored = await chrome.storage.sync.get(['localFolders', 'localFoldersOff']);
        let folders = (stored.localFolders || []).filter(folder => !(stored.localFoldersOff || []).includes(folder));

        // Offline mode: the downloaded favorites (with their tags from the favorites list) and pools too.
        let favoriteTags = new Map();
        if (offlineMode)
        {
            folders = folders.concat(await window.appInfo.offlineFolders());
            let favorites = (await chrome.storage.local.get('personalListItems')).personalListItems || [];
            for (let item of favorites)
                if (item.md5 && typeof item.tags == 'string')
                    favoriteTags.set(item.md5, item.tags.toLowerCase());
        }

        if (folders.length == 0)
            this.sitesManager.displayWarningMessage(offlineMode
                ? 'Offline there are only your folders and downloads. Add folders in Settings → Folders, or turn on the offline copies in Settings → Data usage while online.'
                : 'Add your folders in Settings → Folders to browse them.');

        let files = folders.length > 0 ? await window.appInfo.listLocalMedia(folders) : [];

        // A pool offline: its downloaded folder, "<name> (<number>)", in page order.
        let poolId = poolIdFromSearch(searchText);
        let words = poolId != null ? [] : searchText.toLowerCase().replace(/(^|\s)(?:order|sort):\S+/g, ' ') // sorting is done afterwards
            .replace(/_/g, ' ').split(/\s+/).filter(word => word && word != '*');

        for (let file of files)
        {
            let parts = file.path.split(/[\\\/]/);

            if (poolId != null && !parts.slice(0, -1).some(folder => folder.endsWith('(' + poolId + ')')))
                continue;

            let md5 = parts[parts.length - 1].replace(/^\d+ /, '').replace(/\.[^.]+$/, '');
            let siteTags = favoriteTags.get(md5) || '';
            let searchable = (parts.join(' ') + ' ' + siteTags).toLowerCase().replace(/[_.-]/g, ' ');

            let matches = words.every(word => word.startsWith('-')
                ? !searchable.includes(word.slice(1))
                : searchable.includes(word));

            if (matches)
                this.addSlide(file, parts, siteTags);
        }

        if (poolId != null)
            this.allUnsortedSlides.sort((a, b) => a.fileUrl.localeCompare(b.fileUrl));

        this.lastPageLoaded++;
        this.hasExhaustedSearch = true;

        doneSearchingSiteCallback(this);
    }

    addSlide(file, parts, siteTags = '')
    {
        if (!this.isPathForSupportedMediaType(file.url))
            return;

        // Tags: the folder names, and the words of the file name.
        let fileName = parts.pop().replace(/\.[^.]+$/, '');
        let tags = parts.map(folder => folder.replace(/\s+/g, '_'))
            .concat(fileName.split(/[\s_-]+/))
            .filter(tag => tag)
            .join(' ') + (siteTags ? ' ' + siteTags : '');

        if (this.areSomeTagsAreBlacklisted(tags))
            return;

        // The file's address stands in for the post ID and MD5 (favorites tell items apart by them).
        // Width and height are filled in when the file has loaded (app_settings.js).
        this.allUnsortedSlides.push(new Slide(
            SITE_LOCAL,
            file.url,
            file.url,
            file.url,
            file.url,
            0,
            0,
            new Date(file.modified),
            0,
            this.getMediaTypeFromPath(file.url),
            file.url,
            tags
        ));
    }
}

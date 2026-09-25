// Features added on top of the extension, shared by both pages:
// theme, rebindable hotkeys, the tag panel, touch screen mode and the settings window.
// Settings are stored with chrome.storage like the extension's own.
(function () {
    const isFavoritesPage = document.body.classList.contains('favorites-page');

    // "MSG", from the page title. (The original extension's license doesn't allow
    // publishing under the name "Booru Slideshow".)
    const APP_NAME = document.title.split(' – ')[0];

    // A hotkey is stored as one number: the key code, plus these for held modifier keys.
    const CTRL = 256;
    const SHIFT = 512;
    const ALT = 1024;

    // Most actions map to the key constants in globals.js that the views compare against,
    // so rebinding a key only means changing those constants. The views only look at the
    // key code, so those actions take single keys. Actions marked `app` are handled here
    // and can also use Ctrl/Shift/Alt combinations.
    const HOTKEY_ACTIONS = [
        { id: 'previous', label: 'Previous', defaults: [37, 65], slots: 2 },
        { id: 'next', label: 'Next', defaults: [39, 68], slots: 2 },
        { id: 'back10', label: 'Back 10', defaults: [87], slots: 1 },
        { id: 'forward10', label: 'Forward 10', defaults: [83], slots: 1 },
        { id: 'playPause', label: 'Play / pause', defaults: [32], slots: 1, note: 'Enter also works' },
        { id: 'autoFit', label: 'Toggle auto-fit', defaults: [70], slots: 1 },
        { id: 'openSource', label: 'Open post in browser', defaults: [69], slots: 1 },
        { id: 'download', label: 'Download the image', note: 'To the "downloads" folder in Settings → Folders', defaults: [76], slots: 1 },
        { id: 'favorite', label: isFavoritesPage ? 'Unfave' : 'Fave / unfave', defaults: [71], slots: 1 },
        { id: 'toggleTags', label: 'Show / hide tags', defaults: [82], slots: 1, app: true },
        { id: 'openFavorites', label: isFavoritesPage ? 'Back to the slideshow' : 'Open favorites', note: 'Favorites ⇄ slideshow', defaults: [CTRL + 70], slots: 1, app: true },
        { id: 'openSettings', label: 'Open / close settings', defaults: [CTRL + 83], slots: 1, app: true },
        { id: 'setBackground', label: 'Use the image as background', note: 'Without an image: choose a file', defaults: [CTRL + 76], slots: 1, app: true },
        { id: 'showInterface', label: 'Show the controls', note: 'Scrolls down so the search is at the top; again: back to the image', defaults: [85], slots: 1, app: true },
        { id: 'addToAnalysis', label: 'Add to / remove from the tag analysis', defaults: [67], slots: 1, app: true }
    ];

    // Keys that already do something fixed: Enter (play/pause, search), Tab, Esc, F5, F11, F12,
    // Ctrl+R (reload) and Ctrl+Shift+I (developer tools).
    const RESERVED_KEYS = [9, 13, 27, 116, 122, 123, CTRL + 82, CTRL + SHIFT + 73];
    const MODIFIER_KEYS = [16, 17, 18, 91, 92, 93];
    const NO_KEY = -1;

    const DEFAULTS = {
        appTheme: 'dark',
        appHotkeys: defaultHotkeys(),
        tagsPosition: 'left',
        // Clicking a tag: 'replace' searches for that tag only, 'add' adds it to the search.
        tagClick: 'replace',
        // Favorites: clicking the image opens its post on the site.
        clickOpensPost: true,
        showTags: false,
        touchMode: false,
        backgroundImage: '',
        // A random downloaded favorite as the background at each start (instead of backgroundImage).
        backgroundRandomFavorite: false,
        backgroundRandomGifs: false,
        backgroundRandomVideos: false,
        backgroundBlur: 60,
        glassBlur: 28,
        showTips: true,
        syncE621Favorites: true,
        joiHow: false,
        // e621 pools saved with "Save pool": {id, name, count, cover}.
        savedPools: [],
        poolsSort: 'added',
        // Offline copies: downloaded in the background by main.js, shown instead of the site's files.
        autoDownloadFavorites: false,
        autoDownloadPools: false,
        useLocalCopies: true,
        // A sort menu in the search bar instead of typing order: terms.
        searchSortMenu: false,
        showDownloadButton: true,
        // Settings → Developer, revealed by clicking the logo in About.
        developerMode: false,
        showFps: false,
        showDownloadProgress: false,
        searchSort: '',
        // Settings → Data usage: nothing from the internet, only your folders and the downloads.
        offlineMode: false,
        // Tag analysis: the images added to it, {key, tags, thumb}.
        analysisItems: [],
        analysisTop: 3,
        effectsStyle: 'glass',
        // Refract effects style (js/liquid_glass.js); 100 % is the liquid-glass-js default.
        refractBlur: 5,
        refractStrength: 100,
        refractRipple: 100,
        refractCorner: 100,
        refractTint: 35,
        videoAutoplay: true,
        videoAutoMute: false,
        localFolders: [],
        localFoldersOff: [],
        downloadFolder: '',
        rule34UserId: '',
        rule34ApiKey: '',
        quickSearches: Array.from({ length: 10 }, () => ({ sites: [], tags: '' })),
        // The quick searches as cards with the first picture, on the front page, two to a row.
        quickCards: true
    };

    let settings = JSON.parse(JSON.stringify(DEFAULTS));
    window.clickOpensPost = () => settings.clickOpensPost;
    let hotkeyBeingRecorded = null;

    function defaultHotkeys() {
        let hotkeys = {};
        for (let action of HOTKEY_ACTIONS)
            hotkeys[action.id] = action.defaults.slice();
        return hotkeys;
    }

    function save(key) {
        chrome.storage.sync.set({ [key]: settings[key] });
    }

    function controller() {
        if (typeof slideshowController !== 'undefined' && slideshowController)
            return slideshowController;
        if (typeof personalListController !== 'undefined' && personalListController)
            return personalListController;
        return null;
    }

    function currentSlide() {
        try {
            return controller()._model.getCurrentSlide();
        } catch (e) {
            return null;
        }
    }

    // ---------- Theme ----------

    const lightQuery = matchMedia('(prefers-color-scheme: light)');

    function applyTheme() {
        let theme = settings.appTheme;

        if (theme == 'system')
            theme = lightQuery.matches ? 'light' : 'dark';

        document.documentElement.dataset.theme = theme;
        window.appInfo.themeChanged(theme == 'dark');

        try {
            localStorage.setItem('appTheme', settings.appTheme);
        } catch (e) {}
    }

    lightQuery.addEventListener('change', applyTheme);

    // ---------- Hotkeys ----------

    function keyOf(actionId, slot) {
        let keys = settings.appHotkeys[actionId] || [];
        return keys[slot] != null ? keys[slot] : NO_KEY;
    }

    function applyHotkeys() {
        LEFT_ARROW_KEY_ID = keyOf('previous', 0);
        A_KEY_ID = keyOf('previous', 1);
        RIGHT_ARROW_KEY_ID = keyOf('next', 0);
        D_KEY_ID = keyOf('next', 1);
        W_KEY_ID = keyOf('back10', 0);
        S_KEY_ID = keyOf('forward10', 0);
        SPACE_KEY_ID = keyOf('playPause', 0);
        F_KEY_ID = keyOf('autoFit', 0);
        E_KEY_ID = keyOf('openSource', 0);
        L_KEY_ID = keyOf('download', 0);
        G_KEY_ID = keyOf('favorite', 0);
        // The tag panel replaces the slideshow page's own tag toggle, so its key is handled here.
        R_KEY_ID = NO_KEY;
    }

    function hotkeyOf(e) {
        return e.keyCode + (e.ctrlKey ? CTRL : 0) + (e.shiftKey ? SHIFT : 0) + (e.altKey ? ALT : 0);
    }

    function keyName(hotkey) {
        if (hotkey == NO_KEY)
            return '—';

        let prefix = (hotkey & CTRL ? 'Ctrl+' : '') + (hotkey & SHIFT ? 'Shift+' : '') + (hotkey & ALT ? 'Alt+' : '');
        return prefix + singleKeyName(hotkey & 255);
    }

    function singleKeyName(code) {
        const names = {
            [-1]: '—', 8: 'Backspace', 16: 'Shift', 17: 'Ctrl', 18: 'Alt', 32: 'Space', 33: 'Page Up', 34: 'Page Down',
            35: 'End', 36: 'Home', 37: '←', 38: '↑', 39: '→', 40: '↓', 45: 'Insert', 46: 'Delete',
            186: ';', 187: '=', 188: ',', 189: '-', 190: '.', 191: '/', 192: '`', 219: '[', 220: '\\', 221: ']', 222: "'"
        };

        if (names[code] != null)
            return names[code];
        if ((code >= 48 && code <= 57) || (code >= 65 && code <= 90))
            return String.fromCharCode(code);
        if (code >= 96 && code <= 105)
            return 'Num ' + (code - 96);
        if (code >= 112 && code <= 123)
            return 'F' + (code - 111);
        return 'Key ' + code;
    }

    function setHotkey(actionId, slot, code) {
        let message = '';

        // A key can only do one thing: take it away from any other action first.
        if (code != NO_KEY) {
            for (let action of HOTKEY_ACTIONS) {
                let keys = settings.appHotkeys[action.id];
                for (let i = 0; i < keys.length; i++) {
                    if (keys[i] == code && !(action.id == actionId && i == slot)) {
                        keys[i] = NO_KEY;
                        message = keyName(code) + ' was removed from "' + action.label + '".';
                    }
                }
            }
        }

        settings.appHotkeys[actionId][slot] = code;
        save('appHotkeys');
        applyHotkeys();
        renderHotkeys(message);
    }

    function startRecordingHotkey(button) {
        stopRecordingHotkey();
        hotkeyBeingRecorded = { actionId: button.dataset.action, slot: +button.dataset.slot, button: button };
        button.classList.add('recording');
        button.textContent = 'Press a key…';
    }

    function stopRecordingHotkey() {
        if (!hotkeyBeingRecorded)
            return;
        hotkeyBeingRecorded.button.classList.remove('recording');
        hotkeyBeingRecorded.button.textContent = keyName(keyOf(hotkeyBeingRecorded.actionId, hotkeyBeingRecorded.slot));
        hotkeyBeingRecorded = null;
    }

    function recordHotkey(e) {
        e.preventDefault();
        e.stopImmediatePropagation();

        // Holding Ctrl/Shift/Alt: wait for the key that goes with it.
        if (MODIFIER_KEYS.includes(e.keyCode))
            return;

        let { actionId, slot } = hotkeyBeingRecorded;
        let action = HOTKEY_ACTIONS.find(a => a.id == actionId);
        let hotkey = hotkeyOf(e);

        if (hotkey == 27) { // Esc cancels
            stopRecordingHotkey();
            return;
        }

        hotkeyBeingRecorded = null;

        if (hotkey == 8 || hotkey == 46) // Backspace / Delete clears
            setHotkey(actionId, slot, NO_KEY);
        else if (RESERVED_KEYS.includes(hotkey))
            renderHotkeys(keyName(hotkey) + ' is reserved and can\'t be used.');
        else if (!action.app && hotkey != e.keyCode)
            renderHotkeys('"' + action.label + '" takes a single key, without Ctrl, Shift or Alt.');
        else
            setHotkey(actionId, slot, hotkey);
    }

    const APP_ACTIONS = {
        toggleTags: () => setShowTags(!settings.showTags),
        openFavorites: () => { location.href = isFavoritesPage ? 'slideshow.html' : 'personal_list.html'; },
        openSettings: () => {
            let dialog = document.getElementById('settings-dialog');
            if (dialog.open)
                dialog.close();
            else
                dialog.showModal();
        },
        setBackground: useCurrentImageAsBackground,
        showInterface: () => {
            if (!hasSlide() || window.scrollY + 20 < document.getElementById('content').offsetTop)
                scrollToControls();
            else
                scrollToImage();
        },
        addToAnalysis: () => toggleCurrentInAnalysis()
    };

    // Runs before the pages' own key handlers.
    window.addEventListener('keydown', function (e) {
        if (hotkeyBeingRecorded) {
            recordHotkey(e);
            return;
        }

        let hotkey = hotkeyOf(e);
        let isCombination = e.ctrlKey || e.altKey || e.metaKey;
        let focused = document.activeElement;
        let typing = focused && (focused.tagName == 'INPUT' || focused.tagName == 'TEXTAREA');
        let settingsOpen = document.querySelector('dialog[open]') != null;

        for (let actionId in APP_ACTIONS) {
            if (hotkey != keyOf(actionId, 0))
                continue;

            // Single keys stay free for typing and for the settings window; combinations work everywhere,
            // except that only the settings key itself does something while the settings are open.
            if (!isCombination && (typing || settingsOpen))
                break;
            if (settingsOpen && actionId != 'openSettings')
                break;

            e.preventDefault();
            e.stopImmediatePropagation();
            APP_ACTIONS[actionId]();
            return;
        }

        let quickIndex = QUICK_KEYS.indexOf(hotkey);
        if (quickIndex >= 0 && !isFavoritesPage && !typing && !settingsOpen) {
            e.preventDefault();
            runQuickSearch(quickIndex);
            return;
        }

        // The pages' own hotkeys only check the key code, so keep combinations
        // (like Ctrl+F) from also triggering the plain key's action (F).
        if (isCombination)
            e.stopImmediatePropagation();
    }, true);

    // ---------- Tips while loading ----------

    // Written with the hotkeys the user has set.
    function tips() {
        let k = id => '<kbd>' + keyName(keyOf(id, 0)) + '</kbd>';
        let list = [
            'Scroll down for the search, the next slides and the tags.',
            k('toggleTags') + ' shows the tags. Click a tag to search for it.',
            k('openFavorites') + ' opens your favorites, ' + k('openSettings') + ' the settings.',
            k('setBackground') + ' makes the image the background of the start screen.',
            k('favorite') + (isFavoritesPage ? ' unfaves the image.' : ' faves the image' + (settings.syncE621Favorites ? ', on e621 too when your API key is set.' : '.')),
            k('download') + ' saves the image to Downloads/MSG/downloads.',
            k('playPause') + ' or <kbd>Enter</kbd> plays the slideshow. Settings → Slideshow can let videos play to the end.',
            k('back10') + ' and ' + k('forward10') + ' jump 10 slides at a time.',
            k('previous') + ' and ' + k('next') + ' go to the previous and next slide.',
            k('autoFit') + ' switches auto-fit on and off.',
            k('openSource') + ' opens the post in your browser.',
            'All hotkeys can be changed in Settings → Hotkeys.',
            'Settings → Touch screen: tap the edges to change slides, double-tap to fave.',
            'Settings → Appearance: dark or light theme, glass blur and a background image.',
            'Settings → Favorites imports your favorites from e621, Derpibooru and Gelbooru.',
            'On the favorites page, Random shuffles them, and Settings → Favorites can download them all.',
            'Tags from e621 and Danbooru are grouped by category, in e621\'s colors.',
            '<kbd>F11</kbd> toggles full screen.',
            k('showInterface') + ' brings up the search and the controls; again takes you back to the image.',
            'Settings → Folders: browse your own images and videos, and choose where downloads go.',
            'Settings → Slideshow: choose whether videos play by themselves and start muted.',
            'Settings → Appearance → Effects: Solid turns off the blur and animations.',
            'Keys 1–0 are quick searches: save sites and tags for them in Settings → Quick searches.'
        ];
        return list;
    }

    let tipElement = null;
    let tipShowTimer = null;
    let tipRotateTimer = null;
    let lastTip = -1;

    function showNextTip() {
        let list = tips();
        let index;
        do {
            index = Math.floor(Math.random() * list.length);
        } while (index == lastTip && list.length > 1);
        lastTip = index;

        tipElement.innerHTML = '<span class="tip-label">Tip</span> ' + list[index];
        tipElement.classList.add('visible');
    }

    function loadingChanged() {
        let loading = document.getElementById('loading-animation').style.display == 'inline';

        clearTimeout(tipShowTimer);
        clearInterval(tipRotateTimer);

        if (!loading || !settings.showTips) {
            tipElement.classList.remove('visible');
            return;
        }

        // Only for loads that take a moment, so quick ones don't flash a tip.
        tipShowTimer = setTimeout(function () {
            showNextTip();
            tipRotateTimer = setInterval(showNextTip, 6000);
        }, 700);
    }

    function setupLoadingTips() {
        tipElement = document.createElement('div');
        tipElement.id = 'loading-tip';
        tipElement.className = 'glass';
        document.getElementById('slide-wrapper').appendChild(tipElement);

        new MutationObserver(loadingChanged).observe(document.getElementById('loading-animation'), { attributes: true, attributeFilter: ['style'] });
    }

    // ---------- Background image from the current slide (Ctrl+L) ----------

    async function useCurrentImageAsBackground() {
        let slide = hasSlide() ? currentSlide() : null;

        if (!slide) {
            let url = await window.appInfo.chooseBackground();
            if (url)
                setBackgroundImage(url);
            return;
        }

        showToast('Setting the background image…');

        try {
            setBackgroundImage(await window.appInfo.backgroundFromUrl(slide.fileUrl));
            showToast('This is now the background.');
        } catch (e) {
            showToast('Couldn\'t set the background image: ' + e.message);
        }
    }

    let toastTimer = null;

    function showToast(message) {
        let toast = document.getElementById('toast');

        if (!toast) {
            toast = document.createElement('div');
            toast.id = 'toast';
            toast.className = 'glass';
            // A popover sits in the top layer, so the notice shows over the settings window too.
            toast.popover = 'manual';
            document.body.appendChild(toast);
        }

        toast.textContent = message;

        // Shown again each time, so it stays above a settings window opened after it.
        if (toast.matches(':popover-open'))
            toast.hidePopover();
        toast.showPopover();
        toast.classList.add('visible');

        clearTimeout(toastTimer);
        toastTimer = setTimeout(function () {
            toast.classList.remove('visible');
            toastTimer = setTimeout(() => toast.hidePopover(), 250);
        }, 2500);
    }

    // ---------- Tag panel and background ----------

    let renderScheduled = false;

    function scheduleSlideUpdate() {
        if (renderScheduled)
            return;
        renderScheduled = true;
        requestAnimationFrame(function () {
            renderScheduled = false;
            updateForCurrentSlide();
        });
    }

    function hasSlide() {
        return document.getElementById('navigation').style.display == 'block';
    }

    function updateForCurrentSlide() {
        let slide = hasSlide() ? currentSlide() : null;
        updateAnalysisButtons();
        updateDownloadButton();

        // Before there is anything to show, the page content starts at the top instead of below the image.
        document.body.classList.toggle('has-slide', slide != null);
        updateSearchBeam(slide != null);

        // Blurred, darkened copy of the current slide behind everything, for the glass to pick up.
        // Before there is a slide (e.g. at start), the background image from the settings instead.
        paintBackground(document.getElementById('ambient'), slide ? thumbnailUrl(slide.previewFileUrl || slide.fileUrl) : settings.backgroundImage);

        renderTags(slide);
    }

    // An image as the element's background; a video (only from the random favorite) as a muted, looping <video> in it.
    function paintBackground(element, url) {
        let video = element.querySelector('video');
        if (url && /\.(mp4|webm|m4v|mov|ogv)$/i.test(url)) {
            element.style.backgroundImage = 'none';
            if (!video) {
                video = document.createElement('video');
                video.muted = video.loop = video.autoplay = true;
                element.appendChild(video);
            }
            if (video.getAttribute('src') !== url)
                video.src = url;
        } else {
            if (video)
                video.remove();
            element.style.backgroundImage = url ? 'url("' + url.replace(/"/g, '%22') + '")' : 'none';
        }
    }

    // Tag categories in e621's order, with e621's headings (colors are in app.css).
    const TAG_CATEGORIES = [
        ['invalid', 'Invalid'],
        ['artist', 'Artists'],
        ['contributor', 'Contributors'],
        ['copyright', 'Copyrights'],
        ['character', 'Characters'],
        ['species', 'Species'],
        ['general', 'General'],
        ['meta', 'Meta'],
        ['lore', 'Lore']
    ];

    // Where to look up the categories of a post that was saved without them (e.g. older favorites).
    const TAG_GROUP_SOURCES = {
        [SITE_E621]: {
            url: id => 'https://e621.net/posts/' + id + '.json',
            groups: json => json.post.tags
        },
        [SITE_DANBOORU]: {
            url: id => 'https://danbooru.donmai.us/posts/' + id + '.json',
            groups: json => SiteManagerDanbooru.getTagGroups(json)
        }
    };

    const fetchedTagGroups = new Map();

    async function fetchTagGroups(slide) {
        let key = slide.siteId + ':' + slide.id;

        if (!fetchedTagGroups.has(key)) {
            let source = TAG_GROUP_SOURCES[slide.siteId];
            fetchedTagGroups.set(key, fetch(source.url(slide.id))
                .then(response => response.ok ? response.json() : Promise.reject(response.status))
                .then(source.groups)
                .catch(() => null));
        }

        let groups = await fetchedTagGroups.get(key);

        if (groups) {
            slide.tagGroups = groups;
            rememberTagGroupsInFavorites(slide, groups);
        }

        return groups;
    }

    let saveFavoritesTimer = null;

    // On the favorites page, store fetched categories with the favorite so they are there next time.
    function rememberTagGroupsInFavorites(slide, groups) {
        if (!isFavoritesPage)
            return;

        let model = controller()._model;
        let item = model.personalList.personalListItems.find(i => i.siteId == slide.siteId && i.id == slide.id);

        if (!item || item.tagGroups)
            return;

        item.tagGroups = groups;
        clearTimeout(saveFavoritesTimer);
        saveFavoritesTimer = setTimeout(() => model.dataLoader.savePersonalList(), 10000);
    }

    let renderedTagsKey = null;

    async function renderTags(slide) {
        let tagList = document.getElementById('tag-list');
        let key = slide ? slide.siteId + ':' + slide.id : '';

        if (!document.body.classList.contains('tags-visible') || key === renderedTagsKey)
            return;

        renderedTagsKey = key;

        if (!slide)
            return;

        let groups = slide.tagGroups;

        if (!groups && TAG_GROUP_SOURCES[slide.siteId]) {
            drawTags(tagList, null, slide.tags);
            groups = await fetchTagGroups(slide);

            if (renderedTagsKey !== key)
                return; // moved on to another slide meanwhile
        }

        drawTags(tagList, groups, slide.tags);
    }

    function drawTags(tagList, groups, tagString) {
        tagList.textContent = '';

        let lists = groups
            ? TAG_CATEGORIES.filter(([category]) => groups[category] && groups[category].length > 0)
                .map(([category, heading]) => [category, heading, groups[category]])
            : [['general', 'Tags', (tagString || '').trim().split(/\s+/).filter(t => t)]];

        if (lists.length == 0 || lists[0][2].length == 0) {
            tagList.innerHTML = '<p class="muted">No tags</p>';
            return;
        }

        for (let [category, heading, tags] of lists) {
            let group = document.createElement('section');
            group.className = 'tag-group tag-' + category;

            let title = document.createElement('h5');
            title.textContent = heading;
            group.appendChild(title);

            let list = document.createElement('ul');

            for (let tag of tags) {
                let item = document.createElement('li');
                let link = document.createElement('button');
                link.className = 'tag';
                link.textContent = tag.replace(/_/g, ' ');
                link.title = (isFavoritesPage ? 'Filter by ' : 'Search for ') + tag;
                link.addEventListener('click', () => searchForTag(tag, settings.tagClick == 'add'));
                item.appendChild(link);
                list.appendChild(item);
            }

            group.appendChild(list);
            tagList.appendChild(group);
        }
    }

    function searchForTag(tag, addToSearch = false) {
        let textBox = document.getElementById(isFavoritesPage ? 'filter-text' : 'search-text');
        let words = textBox.value.trim().split(/\s+/).filter(w => w);
        textBox.value = !addToSearch ? tag : words.includes(tag) ? words.join(' ') : words.concat(tag).join(' ');
        textBox.dispatchEvent(new CustomEvent('change'));
        document.getElementById(isFavoritesPage ? 'filter-button' : 'search-button').click();
    }

    function setShowTags(show) {
        settings.showTags = show;
        save('showTags');
        applyLayoutSettings();
    }

    function applyLayoutSettings() {
        document.body.classList.toggle('tags-visible', settings.showTags);
        document.body.dataset.tagsPosition = settings.tagsPosition;
        document.body.classList.toggle('touch-mode', settings.touchMode);
        document.body.classList.toggle('solid-style', settings.effectsStyle == 'solid');
        document.body.classList.toggle('refract-style', settings.effectsStyle == 'refract');
        applyRefract();
        let refractSettings = document.getElementById('refract-settings');
        if (refractSettings)
            refractSettings.hidden = settings.effectsStyle != 'refract';
        document.getElementById('tags-toggle-button').classList.toggle('active', settings.showTags);

        // Left: over the image, scrolling with the page. Below: the last thing on the page.
        let tagPanel = document.getElementById('tag-panel');
        if (settings.tagsPosition == 'below')
            document.getElementById('content').appendChild(tagPanel);
        else
            document.body.insertBefore(tagPanel, document.getElementById('content'));

        renderedTagsKey = null;
        scheduleSlideUpdate();
    }

    function scrollToImage() {
        window.scrollTo({ top: 0, behavior: 'smooth' });
    }

    function scrollToControls() {
        document.getElementById('content').scrollIntoView({ behavior: 'smooth', block: 'start' });
    }

    // ---------- Thumbnails: as many as fit on one row ----------

    const THUMBNAIL_SIZE = 104;
    const THUMBNAIL_GAP = 10;

    function setupThumbnailCount() {
        let wrapper = document.getElementById('thumbnail-list-wrapper');

        new ResizeObserver(function () {
            let width = wrapper.clientWidth;

            if (width == 0)
                return; // hidden

            let count = Math.max(1, Math.floor((width + THUMBNAIL_GAP) / (THUMBNAIL_SIZE + THUMBNAIL_GAP)));
            let c = controller();

            if (!c)
                return;

            // The slideshow page keeps the count in its SitesManager, the favorites page in its model.
            let owner = c._model.sitesManager || c._model;

            if (owner.maxNumberOfThumbnails == count)
                return;

            owner.maxNumberOfThumbnails = count;

            if (hasSlide()) {
                c._view.showThumbnails();
                if (c._model.preloadCurrentAndNextSlides)
                    c._model.preloadCurrentAndNextSlides();
            }
        }).observe(wrapper);
    }

    function watchSlideChanges() {
        let observer = new MutationObserver(scheduleSlideUpdate);
        observer.observe(document.getElementById('current-image'), { attributes: true, attributeFilter: ['src'] });
        observer.observe(document.getElementById('current-video'), { attributes: true, attributeFilter: ['src'] });
        observer.observe(document.getElementById('navigation'), { attributes: true, attributeFilter: ['style'] });
        observer.observe(document.getElementById('current-slide-number'), { childList: true, characterData: true, subtree: true });
    }

    // ---------- Touch screen mode ----------

    let pendingTap = null;
    let lastTapTime = 0;

    function setupTouchMode() {
        let wrapper = document.getElementById('slide-wrapper');

        // Capture phase, so the image's own click (open the post in the browser) doesn't run.
        wrapper.addEventListener('click', function (e) {
            if (!settings.touchMode)
                return;

            // Leave the video's control bar usable.
            let video = document.getElementById('current-video');
            if (e.target == video && e.clientY > video.getBoundingClientRect().bottom - 60)
                return;

            e.preventDefault();
            e.stopPropagation();

            let now = Date.now();

            if (now - lastTapTime < 300) {
                clearTimeout(pendingTap);
                pendingTap = null;
                lastTapTime = 0;
                doubleTap(e.clientX, e.clientY);
                return;
            }

            lastTapTime = now;
            let x = e.clientX / window.innerWidth;

            pendingTap = setTimeout(function () {
                pendingTap = null;

                if (x < 0.3)
                    document.getElementById('previous-button').click();
                else if (x > 0.7)
                    document.getElementById('next-button').click();
                else if (window.scrollY < 50)
                    scrollToControls();
                else
                    scrollToImage();
            }, 280);
        }, true);
    }

    function doubleTap(x, y) {
        let favoriteButton = document.getElementById('favorite-button');

        // Only the slideshow page can fave; everything on the favorites page already is one.
        if (!favoriteButton || document.getElementById('navigation').style.display == 'none')
            return;

        // The heart appears where the tap was (see showFavoriteHeart).
        lastTapPoint = { x: x, y: y, time: Date.now() };
        favoriteButton.click();
    }

    // ---------- Favorite: heart effect and state ----------

    let lastTapPoint = null;

    // On every fave or unfave (button, hotkey, double-tap): a heart in the middle of the
    // window, or where a double-tap was. White when unfaved.
    function showFavoriteHeart(faved) {
        let point = lastTapPoint && Date.now() - lastTapPoint.time < 500
            ? lastTapPoint
            : { x: window.innerWidth / 2, y: window.innerHeight / 2 };
        lastTapPoint = null;

        let heart = document.createElement('div');
        heart.className = 'tap-heart' + (faved ? '' : ' unfaved');
        heart.textContent = '♥';
        heart.style.left = point.x + 'px';
        heart.style.top = point.y + 'px';
        document.body.appendChild(heart);
        setTimeout(function () { heart.remove(); }, 800);
    }

    // The player card (navigation and next slides) shows whether the image is a favorite,
    // following the ♥ button that the slideshow page keeps up to date.
    function watchFavoriteState() {
        let favoriteButton = document.getElementById('favorite-button');

        if (!favoriteButton)
            return; // favorites page: everything is a favorite

        let update = function () {
            document.body.classList.toggle('current-faved', favoriteButton.classList.contains('faved'));
        };

        new MutationObserver(update).observe(favoriteButton, { attributes: true, attributeFilter: ['class'] });
        update();
    }

    // ---------- Search bar beam: only until the first search ----------

    // The rainbow beam on the slideshow page's search bar invites a search; once there are
    // slides it fades out, and it comes back if the page is empty again.
    function updateSearchBeam(hasSlides) {
        let search = document.querySelector('#search[data-beam]');

        if (!search || search.hasAttribute('data-active') == !hasSlides)
            return;

        if (hasSlides) {
            search.removeAttribute('data-active');
            search.setAttribute('data-fading', '');
            setTimeout(() => search.removeAttribute('data-fading'), 600);
        } else {
            search.removeAttribute('data-fading');
            search.setAttribute('data-active', '');
        }
    }

    // The beam's endless animation stands still while the window is in the background.
    function pauseSearchBeam() {
        let search = document.querySelector('#search[data-beam]');

        if (search)
            search.toggleAttribute('data-paused', document.hidden || !document.hasFocus());
    }

    window.addEventListener('focus', pauseSearchBeam);
    window.addEventListener('blur', pauseSearchBeam);
    document.addEventListener('visibilitychange', pauseSearchBeam);
    pauseSearchBeam();

    // ---------- Search history menu ----------

    // A glass menu in place of the browser's datalist popup, which can't be styled.
    // The view still fills the datalist; this only shows its options.
    function setupSearchHistoryMenu() {
        let input = document.getElementById('search-text');
        let menu = document.getElementById('search-history-menu');

        if (!menu)
            return; // favorites page

        let active = -1;

        function hide() {
            active = -1;
            if (menu.matches(':popover-open'))
                menu.hidePopover();
        }

        function pick(text) {
            input.value = text;
            input.dispatchEvent(new CustomEvent('change'));
            hide();
        }

        function show() {
            let text = input.value.trim().toLowerCase();
            let items = [...new Set([...document.querySelectorAll('#search-history option')].map(o => o.value.trim()))]
                .filter(v => v.toLowerCase().includes(text) && v.toLowerCase() != text).slice(0, 8);

            if (items.length == 0)
                return hide();

            active = -1;
            menu.replaceChildren(...items.map(value => {
                let li = document.createElement('li');
                li.textContent = value;
                li.addEventListener('mousedown', e => {
                    e.preventDefault(); // keeps the focus in the box
                    pick(value);
                    document.getElementById('search-button').click();
                });
                return li;
            }));

            if (!menu.matches(':popover-open'))
                menu.showPopover();

            // Below the box, or above it when the search card is near the bottom of the window.
            let box = input.getBoundingClientRect();
            menu.style.left = box.left + 'px';
            menu.style.width = box.width + 'px';
            let below = window.innerHeight - box.bottom;
            menu.style.top = (below >= menu.offsetHeight + 8 || below > box.top ? box.bottom + 6 : box.top - 6 - menu.offsetHeight) + 'px';
        }

        function highlight(index) {
            let items = menu.children;
            active = (index + items.length) % items.length;
            [...items].forEach((li, i) => li.classList.toggle('active', i == active));
        }

        input.addEventListener('focus', show);
        input.addEventListener('click', show);
        input.addEventListener('input', show);
        input.addEventListener('blur', hide);
        window.addEventListener('scroll', hide, { passive: true });
        window.addEventListener('resize', hide);

        input.addEventListener('keydown', e => {
            if (!menu.matches(':popover-open'))
                return;

            if (e.key == 'ArrowDown' || e.key == 'ArrowUp') {
                e.preventDefault();
                highlight(active + (e.key == 'ArrowDown' ? 1 : -1));
            }
            else if (e.key == 'Enter' && active >= 0) {
                pick(menu.children[active].textContent); // the Enter itself still starts the search
            }
            else if (e.key == 'Enter' || e.key == 'Escape') {
                if (e.key == 'Escape')
                    e.stopPropagation();
                hide();
            }
        });
    }

    // ---------- Rainbow frame around an image that is a favorite ----------

    function setupFavoriteFrame() {
        let frame = document.getElementById('fave-beam');

        if (!frame)
            return; // favorites page

        let image = document.getElementById('current-image');
        let video = document.getElementById('current-video');

        let place = function () {
            let media = video.style.display == 'inline' ? video : image;
            let shown = document.body.classList.contains('current-faved') && hasSlide() && media.style.display == 'inline';
            let rect = media.getBoundingClientRect();

            if (shown && rect.width > 0) {
                frame.style.left = rect.left + 'px';
                frame.style.top = rect.top + 'px';
                frame.style.width = rect.width + 'px';
                frame.style.height = rect.height + 'px';
            }

            if (shown && !frame.hasAttribute('data-active')) {
                frame.removeAttribute('data-fading');
                frame.setAttribute('data-active', '');
            } else if (!shown && frame.hasAttribute('data-active')) {
                frame.removeAttribute('data-active');
                frame.setAttribute('data-fading', '');
                setTimeout(() => frame.removeAttribute('data-fading'), 500);
            }
        };

        new ResizeObserver(place).observe(image);
        new ResizeObserver(place).observe(video);
        new MutationObserver(place).observe(document.body, { attributes: true, attributeFilter: ['class'] });
        new MutationObserver(place).observe(image, { attributes: true, attributeFilter: ['style', 'src'] });
        new MutationObserver(place).observe(video, { attributes: true, attributeFilter: ['style', 'src'] });
        image.addEventListener('load', place);
        video.addEventListener('loadedmetadata', place);
        window.addEventListener('resize', place);
    }

    // ---------- Files from your own folders: their size ----------

    // Files from your folders come without a width and height (see site_manager_local.js);
    // take them from the file once it has loaded, so auto-fit can size it.
    function setupLocalFileSizes() {
        let fill = function (width, height) {
            let slide = currentSlide();

            if (!slide || slide.width || !width)
                return;

            slide.width = width;
            slide.height = height;
            controller()._view.updateSlideSize();
        };

        let image = document.getElementById('current-image');
        let video = document.getElementById('current-video');
        image.addEventListener('load', () => fill(image.naturalWidth, image.naturalHeight));
        video.addEventListener('loadedmetadata', () => fill(video.videoWidth, video.videoHeight));
    }

    // ---------- Videos: play automatically, start muted ----------

    function setupVideoSettings() {
        let video = document.getElementById('current-video');

        // Runs right after the page sets a new video, before it has loaded enough to start.
        new MutationObserver(function () {
            if (!video.getAttribute('src'))
                return;

            video.autoplay = settings.videoAutoplay;

            if (settings.videoAutoMute)
                video.muted = true;
        }).observe(video, { attributes: true, attributeFilter: ['src'] });
    }

    function renderVideoSettings() {
        let section = document.querySelector('.settings-section[data-section="slideshow"]');
        let box = section.querySelector('#video-settings');

        if (!box) {
            box = document.createElement('div');
            box.id = 'video-settings';
            section.appendChild(box);
        }

        box.innerHTML =
            '<h4 class="subheading">Videos</h4>' +
            '<ul class="settings-list">' +
                '<li><label class="row"><span>Play automatically<small>Start videos as soon as they are shown. When off, press play on the video.</small></span>' +
                    '<input type="checkbox" id="video-autoplay"' + (settings.videoAutoplay ? ' checked' : '') + '></label></li>' +
                '<li><label class="row"><span>Start muted<small>Every video starts without sound; the sound can still be turned on.</small></span>' +
                    '<input type="checkbox" id="video-auto-mute"' + (settings.videoAutoMute ? ' checked' : '') + '></label></li>' +
            '</ul>';

        box.querySelector('#video-autoplay').addEventListener('change', function (e) {
            settings.videoAutoplay = e.target.checked;
            save('videoAutoplay');
        });

        box.querySelector('#video-auto-mute').addEventListener('change', function (e) {
            settings.videoAutoMute = e.target.checked;
            save('videoAutoMute');
        });
    }

    // ---------- Folders: your own folders and the download folder ----------

    // For text and attribute values (value="…") in HTML strings.
    function escapeHtml(text) {
        return String(text).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
    }

    function renderFolders() {
        let section = document.querySelector('.settings-section[data-section="folders"]');
        let folders = settings.localFolders;

        section.innerHTML =
            '<p class="section-intro">Browse your own images and videos: add folders here, then turn on <b>Your folders</b> in Sites &amp; accounts' +
                (isFavoritesPage ? ' (on the slideshow page)' : '') +
                ' and search by folder or file name, or leave the search empty to see everything. Subfolders are included. Untick a folder to leave it out of the search.</p>' +
            '<ul class="settings-list">' +
                (folders.length == 0
                    ? '<li class="row"><span class="muted">No folders yet</span></li>'
                    : folders.map((folder, i) =>
                        '<li class="row"><input type="checkbox" data-folder-on="' + i + '" title="Include in the search"' + (settings.localFoldersOff.includes(folder) ? '' : ' checked') + '>' +
                        '<span class="folder-path">' + escapeHtml(folder) + '</span><button data-remove-folder="' + i + '">Remove</button></li>').join('')) +
            '</ul>' +
            '<div class="section-footer"><button id="add-local-folder" class="primary">+ Add folder…</button></div>' +

            '<h4 class="subheading">Downloads</h4>' +
            '<ul class="settings-list">' +
                '<li class="row stacked"><span>Download folder<small>Where ' + keyName(keyOf('download', 0)) + ' and the Download button save files. They go into a "downloads" folder in it; favorites go into "favorites".</small>' +
                    '<span class="folder-path">' + escapeHtml(settings.downloadFolder || 'Downloads\\MSG (default)') + '</span></span>' +
                    '<div class="key-buttons"><button id="choose-download-folder">Choose…</button>' +
                    (settings.downloadFolder ? '<button id="reset-download-folder">Use default</button>' : '') + '</div></li>' +
            '</ul>';

        section.querySelector('#add-local-folder').addEventListener('click', async function () {
            let folder = await window.appInfo.chooseFolder('Choose a folder to browse');

            if (folder && !settings.localFolders.includes(folder)) {
                settings.localFolders.push(folder);
                save('localFolders');
                renderFolders();
            }
        });

        section.querySelectorAll('[data-folder-on]').forEach(function (box) {
            box.addEventListener('change', function () {
                let folder = settings.localFolders[+box.dataset.folderOn];
                settings.localFoldersOff = settings.localFoldersOff.filter(f => f != folder);
                if (!box.checked)
                    settings.localFoldersOff.push(folder);
                save('localFoldersOff');
            });
        });

        section.querySelectorAll('[data-remove-folder]').forEach(function (button) {
            button.addEventListener('click', function () {
                settings.localFolders.splice(+button.dataset.removeFolder, 1);
                save('localFolders');
                renderFolders();
            });
        });

        section.querySelector('#choose-download-folder').addEventListener('click', async function () {
            let folder = await window.appInfo.chooseFolder('Choose the download folder');

            if (folder) {
                settings.downloadFolder = folder;
                save('downloadFolder');
                loadLocalCopies();
                renderFolders();
            }
        });

        let reset = section.querySelector('#reset-download-folder');
        if (reset) {
            reset.addEventListener('click', function () {
                settings.downloadFolder = '';
                save('downloadFolder');
                loadLocalCopies();
                renderFolders();
            });
        }
    }

    // ---------- Site logins (Sites & accounts) ----------

    // Rule34 and Gelbooru show the login as one line, "&api_key=<key>&user_id=<number>".
    // Returns its two parts, or null when the text isn't such a line.
    function parseLoginLine(text) {
        if (!/api_key=|user_id=/.test(text || ''))
            return null;

        let parts = new URLSearchParams(text.trim().replace(/^[?&]/, ''));
        return { userId: (parts.get('user_id') || '').trim(), apiKey: (parts.get('api_key') || '').trim() };
    }

    // Checks a site again after its login changed, and marks it offline or not.
    function recheckSite(siteId) {
        let siteManager = controller()._model.sitesManager.siteManagers.find(m => m.id == siteId);
        let checkbox = document.querySelector('input[name="sites-to-search"][value="' + siteId + '"]');

        siteManager.isOnline = false;
        siteManager.pingStatus(function () {
            checkbox.parentElement.classList.toggle('siteOffline', !siteManager.isOnline);
        });
    }

    // Gelbooru's fields are the extension's own (saved through the page's model); this splits a pasted
    // login line into them, fixes values saved that way, and checks Gelbooru again after a change.
    async function setupGelbooruAccount() {
        let idField = document.getElementById('gelb-userid');

        if (!idField)
            return; // favorites page

        let model = controller()._model;

        let applyLine = function (text) {
            let login = parseLoginLine(text);

            if (!login)
                return false;
            if (login.userId)
                model.setGelbUserId(login.userId);
            model.setGelbApiKey(login.apiKey);
            return true;
        };

        for (let container of ['gelb-userid-container', 'gelb-api-key-container']) {
            // Capture phase: runs before the field's own save, which it replaces for a pasted line.
            document.getElementById(container).addEventListener('change', function (e) {
                if (applyLine(e.target.value))
                    e.stopPropagation();
                setTimeout(() => recheckSite(SITE_GELBOORU));
            }, true);
        }

        let saved = await chrome.storage.sync.get(['gelbUserId', 'gelbApiKey']);
        if (applyLine(saved.gelbApiKey) || applyLine(saved.gelbUserId))
            recheckSite(SITE_GELBOORU);
    }

    // ---------- Rule34 login (Sites & accounts) ----------

    // Rule34's API needs a user ID and API key. The page's model reads them from its settings
    // at start (data_loader.js); changes here are passed on directly and Rule34 is checked again.
    function setupRule34Account() {
        let checkbox = document.querySelector('input[name="sites-to-search"][value="RULE"]');

        if (!checkbox)
            return; // favorites page

        // Rule34 shows the login as one string, "&api_key=<key>&user_id=<number>". Whatever is pasted
        // (into either field) is split into the two; this also fixes values saved that way before.
        let applyPasted = function (text) {
            let login = parseLoginLine(text);

            if (!login)
                return false;
            if (login.userId)
                settings.rule34UserId = login.userId;
            if (login.apiKey || parseLoginLine(settings.rule34ApiKey))
                settings.rule34ApiKey = login.apiKey;
            return true;
        };

        let fixedSaved = applyPasted(settings.rule34ApiKey) | applyPasted(settings.rule34UserId);

        checkbox.closest('li').insertAdjacentHTML('afterend',
            '<li id="rule34-account" class="account-fields">' +
                '<input type="text" id="rule34-user-id" placeholder="Rule34 user ID (a number)">' +
                '<input type="password" id="rule34-api-key" placeholder="Rule34 API key">' +
                '<a href="https://rule34.xxx/index.php?page=account&s=options" target="_blank">Get an API key</a>' +
            '</li>' +
            '<li id="rule34-help" class="account-fields"><small class="muted">You can paste the whole "&amp;api_key=…&amp;user_id=…" line from rule34.xxx → My Account → Options into either field.</small></li>');

        let idField = document.getElementById('rule34-user-id');
        let keyField = document.getElementById('rule34-api-key');

        let show = function () {
            idField.value = settings.rule34UserId;
            keyField.value = settings.rule34ApiKey;
        };
        show();

        let check = function () {
            let model = controller()._model;

            model.rule34UserId = settings.rule34UserId;
            model.rule34ApiKey = settings.rule34ApiKey;
            recheckSite(SITE_RULE34);

            if (settings.rule34UserId && !/^\d+$/.test(settings.rule34UserId))
                showToast('The Rule34 user ID is a number (it is in the "user_id=" part on rule34.xxx), not the user name.');
            else if (settings.rule34UserId && !settings.rule34ApiKey)
                showToast('The Rule34 API key is still missing.');
        };

        for (let [field, key] of [[idField, 'rule34UserId'], [keyField, 'rule34ApiKey']]) {
            field.addEventListener('change', function () {
                if (!applyPasted(field.value))
                    settings[key] = field.value.trim();
                save('rule34UserId');
                save('rule34ApiKey');
                show();
                check();
            });
        }

        if (fixedSaved) {
            save('rule34UserId');
            save('rule34ApiKey');
            check();
        }
    }

    // ---------- Quick searches (keys 1–0, slideshow page) ----------

    // Each key 1, 2, … 9, 0 keeps a set of sites and, if wanted, tags. Pressing it selects
    // those sites and searches; without saved tags it searches for what is in the search box.
    const QUICK_KEYS = [49, 50, 51, 52, 53, 54, 55, 56, 57, 48];

    function siteCheckboxes() {
        return [...document.getElementsByName('sites-to-search')];
    }

    function siteName(checkbox) {
        return checkbox.parentElement.querySelector('span').firstChild.textContent.trim();
    }

    function runQuickSearch(index) {
        let quick = settings.quickSearches[index];
        let number = (index + 1) % 10;

        if (!quick || quick.sites.length == 0) {
            showToast('Quick search ' + number + ' is empty. Set it in Settings → Quick searches.');
            return;
        }

        for (let checkbox of siteCheckboxes()) {
            if (checkbox.checked != quick.sites.includes(checkbox.value))
                checkbox.click();
        }

        let searchText = document.getElementById('search-text');
        if (quick.tags) {
            searchText.value = quick.tags;
            searchText.dispatchEvent(new CustomEvent('change'));
        }

        document.getElementById('search-button').click();

        let names = siteCheckboxes().filter(c => quick.sites.includes(c.value)).map(siteName);
        showToast('Quick search ' + number + ': ' + names.join(', ') + (searchText.value ? ' · ' + searchText.value : ''));
    }

    function renderQuickSearches() {
        let section = document.querySelector('.settings-section[data-section="quick"]');

        if (!section)
            return; // favorites page

        let sites = siteCheckboxes();

        section.innerHTML =
            '<p class="section-intro">Keys 1–0 on the slideshow page select the sites saved here and search. ' +
                'Tags are optional: without them the key searches for what is in the search box. ' +
                '"Use current" saves the sites selected now and the search box.</p>' +
            '<ul class="settings-list">' +
                settings.quickSearches.map(function (quick, i) {
                    return '<li class="quick-row" data-quick="' + i + '">' +
                        '<kbd>' + ((i + 1) % 10) + '</kbd>' +
                        '<div class="quick-sites">' + sites.map(c =>
                            '<button class="quick-site' + (quick.sites.includes(c.value) ? ' active' : '') + '" data-site="' + c.value + '">' + escapeHtml(siteName(c).split('.')[0]) + '</button>').join('') +
                        '</div>' +
                        '<input type="text" class="quick-tags" placeholder="Tags (optional)" value="' + escapeHtml(quick.tags) + '">' +
                        '<button class="quick-current" title="Save the sites selected now and the search box">Use current</button>' +
                    '</li>';
                }).join('') +
            '</ul>';

        section.querySelectorAll('.quick-row').forEach(function (row) {
            let quick = settings.quickSearches[+row.dataset.quick];

            row.querySelectorAll('.quick-site').forEach(function (button) {
                button.addEventListener('click', function () {
                    let site = button.dataset.site;
                    quick.sites = quick.sites.includes(site) ? quick.sites.filter(s => s != site) : quick.sites.concat(site);
                    button.classList.toggle('active', quick.sites.includes(site));
                    save('quickSearches');
                    renderQuickCards();
                });
            });

            row.querySelector('.quick-tags').addEventListener('change', function (e) {
                quick.tags = e.target.value.trim();
                save('quickSearches');
                renderQuickCards();
            });

            row.querySelector('.quick-current').addEventListener('click', function () {
                quick.sites = siteCheckboxes().filter(c => c.checked).map(c => c.value);
                quick.tags = document.getElementById('search-text').value.trim();
                save('quickSearches');
                renderQuickSearches();
                renderQuickCards();
            });
        });
    }

    // ---------- Quick search cards (front page) ----------

    // Each quick search as a card with the first picture of its results and, in the wide layout,
    // the main tags. One request at a time, spaced out, and remembered for a while.
    const QUICK_CARD_TTL = 10 * 60 * 1000;
    const QUICK_CARD_GAP = 700;
    const quickCardCache = new Map();
    // The first request waits until the model has loaded the blacklist and ratings.
    // ponytail: a fixed wait, an event from the model if it ever loads slower than this.
    let quickCardQueue = new Promise(resolve => setTimeout(resolve, 1500));
    let quickCardRun = 0;

    // The first slide of the quick search's first site, or null. {slide, cached}
    function fetchQuickSlide(quick) {
        let siteId = quick.sites[0];
        let query = applySearchSort(quick.tags, [siteId], true);
        let key = siteId + '|' + query;
        let hit = quickCardCache.get(key);

        if (hit && Date.now() - hit.time < QUICK_CARD_TTL)
            return Promise.resolve({ slide: hit.slide, cached: true });

        return new Promise(function (resolve) {
            // Its own manager: the live ones share one request that a new one cancels.
            let manager = SiteManagerFactory.createSiteManager({ model: controller()._model, displayWarningMessage() {} }, siteId, 5);
            let timer = setTimeout(() => resolve({ slide: null, cached: false }), 35000);

            manager.isOnline = true;
            manager.enable();
            manager.performSearch(query, function () {
                clearTimeout(timer);
                let slide = manager.allUnsortedSlides[0] || null;
                if (!manager.ranIntoErrorWhileSearching)
                    quickCardCache.set(key, { slide: slide, time: Date.now() });
                resolve({ slide: slide, cached: false });
            });
        });
    }

    function queueQuickSlide(quick, run) {
        let result = quickCardQueue.then(async function () {
            if (run != quickCardRun || !quick.tags || quick.sites[0] == SITE_LOCAL || offlineMode)
                return null;
            let { slide, cached } = await fetchQuickSlide(quick);
            if (!cached)
                await new Promise(resolve => setTimeout(resolve, QUICK_CARD_GAP));
            return slide;
        }).catch(() => null);
        quickCardQueue = result;
        return result;
    }

    function isHttpUrl(url) {
        return /^https?:\/\//i.test(url || '');
    }

    function quickCardTags(slide) {
        let groups = slide.tagGroups || {};
        let tags = [];

        // Most important first: the card shows as many as fit (fitQuickTags).
        for (let [category, count] of [['artist', 3], ['character', 4], ['copyright', 3], ['species', 2]])
            for (let tag of (groups[category] || []).slice(0, count))
                tags.push([category, tag]);

        return tags;
    }

    // The picture's shape for an image of this width / height: a compact 1.15 unless that would crop
    // more than 13 % of the image, so a narrow or a wide image is cut only a little at the edges.
    function quickPictureRatio(ratio) {
        return Math.min(Math.max(1.15, ratio * 0.87), ratio / 0.87);
    }

    // Keeps the tags that fit in the space the card has left, from the first on; the rest are hidden.
    function fitQuickTags(list) {
        let full = false;

        for (let tag of list.children) {
            tag.hidden = false;
            full = full || tag.offsetTop + tag.offsetHeight > list.clientHeight;
            tag.hidden = full;
        }
    }

    function renderQuickCards() {
        let section = document.getElementById('quick-cards');

        if (!section)
            return; // favorites page

        let run = ++quickCardRun;
        let cards = settings.quickSearches.map((quick, index) => ({ quick, index })).filter(card => card.quick.sites.length);

        section.hidden = !settings.quickCards || cards.length == 0;
        section.replaceChildren();
        if (section.hidden)
            return;

        // Static markup only: nothing from the sites goes in with innerHTML.
        let header = document.createElement('div');
        header.className = 'quick-cards-header';
        header.innerHTML = '<h3>Quick searches</h3>' +
            '<button class="glass-button icon-button" id="quick-cards-refresh" title="Load the pictures again">&#8635;</button>';
        section.append(header);

        header.querySelector('#quick-cards-refresh').addEventListener('click', function () {
            quickCardCache.clear();
            renderQuickCards();
        });

        let grid = document.createElement('div');
        grid.className = 'quick-cards-grid';
        section.append(grid);

        for (let { quick, index } of cards) {
            let card = document.createElement('button');
            card.className = 'quick-card';
            card.type = 'button';

            let picture = document.createElement('div');
            picture.className = 'quick-card-picture';
            card.append(picture);

            let number = document.createElement('kbd');
            number.textContent = (index + 1) % 10;
            picture.append(number);

            let info = document.createElement('div');
            info.className = 'quick-card-info';
            let names = siteCheckboxes().filter(c => quick.sites.includes(c.value)).map(c => siteName(c).split('.')[0]);
            let label = document.createElement('span');
            label.className = 'quick-card-query';
            label.textContent = quick.tags || 'The search box';
            let sites = document.createElement('small');
            sites.textContent = names.join(', ');
            info.append(label, sites);
            card.append(info);

            card.title = 'Search ' + (quick.tags || 'what is in the search box') + ' (' + names.join(', ') + ')';
            card.addEventListener('click', () => runQuickSearch(index));
            grid.append(card);

            queueQuickSlide(quick, run).then(function (slide) {
                if (!slide)
                    return;

                let url = slide.sampleFileUrl || slide.previewFileUrl;
                let ratio = Number(slide.width) / Number(slide.height);
                if (ratio > 0 && isFinite(ratio))
                    picture.style.setProperty('--r', quickPictureRatio(ratio));

                if (isHttpUrl(url)) {
                    let image = document.createElement('img');
                    image.loading = 'lazy';
                    image.alt = '';
                    image.src = url;
                    picture.prepend(image);
                }

                let tagList = document.createElement('div');
                tagList.className = 'quick-card-tags';
                for (let [category, name] of quickCardTags(slide)) {
                    let tag = document.createElement('span');
                    tag.className = 'quick-tag tag-' + category;
                    tag.textContent = name.replace(/_/g, ' ');
                    tagList.append(tag);
                }
                if (tagList.childElementCount) {
                    info.append(tagList);
                    new ResizeObserver(() => fitQuickTags(tagList)).observe(tagList);
                }
            });
        }
    }

    // ---------- Settings window ----------

    function segmented(name, options, value) {
        return '<div class="segmented" role="radiogroup">' + options.map(function (option) {
            return '<label><input type="radio" name="' + name + '" value="' + option[0] + '"' +
                (option[0] == value ? ' checked' : '') + '><span>' + option[1] + '</span></label>';
        }).join('') + '</div>';
    }

    function sliderRow(id, title, description, value, max, unit = ' px') {
        return '<li class="row"><span>' + title + '<small>' + description + '</small></span>' +
            '<div class="slider"><input type="range" id="' + id + '" min="0" max="' + max + '" step="1" value="' + value + '">' +
            '<output id="' + id + '-value">' + value + unit + '</output></div></li>';
    }

    // While a blur slider is being moved, the settings window steps aside so the change can be seen.
    // For the background blur, the background image itself is shown (the slide steps aside too).
    function setupBlurSlider(section, id, settingKey, label, apply, showBackground, unit = ' px') {
        let slider = section.querySelector('#' + id);
        let peekTimer = null;

        let peek = function () {
            document.body.classList.add('settings-peek');
            if (showBackground)
                showBackgroundPreview(true);
        };

        let endPeek = function () {
            clearTimeout(peekTimer);
            document.body.classList.remove('settings-peek');
            if (showBackground)
                showBackgroundPreview(false);
        };

        slider.addEventListener('pointerdown', function () {
            peek();
            window.addEventListener('pointerup', endPeek, { once: true });
        });

        slider.addEventListener('input', function () {
            settings[settingKey] = +slider.value;
            section.querySelector('#' + id + '-value').textContent = slider.value + unit;
            apply();
            showToast(label + ': ' + slider.value + unit);

            // Arrow keys: peek for a moment after each step.
            if (!document.body.classList.contains('settings-peek') || peekTimer) {
                peek();
                clearTimeout(peekTimer);
                peekTimer = setTimeout(function () { peekTimer = null; endPeek(); }, 1200);
            }
        });

        slider.addEventListener('change', function () {
            save(settingKey);
        });
    }

    function showBackgroundPreview(on) {
        document.body.classList.toggle('background-preview', on);

        if (on)
            paintBackground(document.getElementById('ambient'), settings.backgroundImage);
        else
            updateForCurrentSlide();
    }

    // The random favorite is picked at start; after changing its settings, pick (or drop it) now.
    async function applyRandomBackground(section) {
        let saved = (await chrome.storage.sync.get('backgroundImage')).backgroundImage || '';
        settings.backgroundImage = settings.backgroundRandomFavorite
            ? await window.appInfo.randomFavoriteImage(settings.backgroundRandomGifs, settings.backgroundRandomVideos) || saved
            : saved;
        paintBackground(section.querySelector('#background-preview'), settings.backgroundImage);
        updateForCurrentSlide();
    }

    function renderAppearance() {
        let section = document.querySelector('.settings-section[data-section="appearance"]');

        section.innerHTML =
            '<ul class="settings-list">' +
                '<li class="row stacked"><span>Theme<small>"System" follows the Windows light/dark setting.</small></span>' +
                    segmented('app-theme', [['dark', 'Dark'], ['light', 'Light'], ['system', 'System']], settings.appTheme) + '</li>' +
                '<li class="row stacked"><span>Effects<small>Refract (test) bends the image at the glass edges like real glass. Solid turns off the glass blur and the animations (the loading orb still turns).</small></span>' +
                    segmented('effects-style', [['glass', 'Glass'], ['refract', 'Refract (test)'], ['solid', 'Solid']], settings.effectsStyle) + '</li>' +
                sliderRow('glass-blur', 'Glass blur', 'How much the cards, the navigation and this window blur what is behind them.', settings.glassBlur, 60) +
            '</ul>' +

            '<ul class="settings-list" id="refract-settings"' + (settings.effectsStyle == 'refract' ? '' : ' hidden') + '>' +
                sliderRow('refract-blur', 'Refract: blur', 'How blurred the image behind the glass cards is.', settings.refractBlur, 20) +
                sliderRow('refract-strength', 'Refract: bending', 'How strongly the edges of the glass bend the image.', settings.refractStrength, 300, ' %') +
                sliderRow('refract-ripple', 'Refract: ripple', 'The wavy lines along the edges.', settings.refractRipple, 300, ' %') +
                sliderRow('refract-corner', 'Refract: corners', 'Extra bending in the corners.', settings.refractCorner, 300, ' %') +
                sliderRow('refract-tint', 'Refract: tint', 'How much the glass is tinted with the theme color. More makes the text easier to read.', settings.refractTint, 100, ' %') +
            '</ul>' +

            '<ul class="settings-list">' +
                '<li><label class="row"><span>Sort menu in the search bar<small>Choose Newest, Oldest, Highest or Lowest score from a menu instead of typing order: terms. Default searches exactly what you type.</small></span>' +
                    '<input type="checkbox" id="search-sort-menu"' + (settings.searchSortMenu ? ' checked' : '') + '></label></li>' +
                (isFavoritesPage ? '' :
                    '<li><label class="row"><span>Quick search cards<small>The quick searches as cards on the front page, each with the first picture of its results. Set them in Quick searches.</small></span>' +
                        '<input type="checkbox" id="quick-cards-toggle"' + (settings.quickCards ? ' checked' : '') + '></label></li>') +
                '<li><label class="row"><span>Download button<small>A Download button in the toolbar that saves the image or video on the screen to the download folder.</small></span>' +
                    '<input type="checkbox" id="show-download-button"' + (settings.showDownloadButton ? ' checked' : '') + '></label></li>' +
                '<li><label class="row"><span>Show tips while loading<small>Tips about the app\'s features, shown while an image is loading.</small></span>' +
                    '<input type="checkbox" id="app-show-tips"' + (settings.showTips ? ' checked' : '') + '></label></li>' +
                (isFavoritesPage ?
                    '<li><label class="row"><span>Click the image to open its post<small>Opens the post on the site in your browser.</small></span>' +
                        '<input type="checkbox" id="app-click-opens-post"' + (settings.clickOpensPost ? ' checked' : '') + '></label></li>' : '') +
            '</ul>' +

            '<h4 class="subheading">Tags</h4>' +
            '<ul class="settings-list">' +
                '<li><label class="row"><span>Show tags<small>Also with the ' + keyName(keyOf('toggleTags', 0)) + ' key or the # button.</small></span>' +
                    '<input type="checkbox" id="app-show-tags"' + (settings.showTags ? ' checked' : '') + '></label></li>' +
                '<li class="row stacked"><span>Tag position<small>Click a tag to ' + (isFavoritesPage ? 'filter by it' : 'search for it') + '.</small></span>' +
                    segmented('tags-position', [['left', 'Left, over the image'], ['below', 'At the bottom']], settings.tagsPosition) + '</li>' +
                '<li class="row stacked"><span>Clicking a tag<small>' + (isFavoritesPage ? 'Filter' : 'Search') + ' for that tag alone, or add it to what is in the ' + (isFavoritesPage ? 'filter' : 'search') + ' box now.</small></span>' +
                    segmented('tag-click', [['replace', 'Only that tag'], ['add', 'Add to the search']], settings.tagClick) + '</li>' +
            '</ul>' +

            '<h4 class="subheading">Background image</h4>' +
            '<ul class="settings-list">' +
                '<li class="row stacked"><span>Image<small>Shown when the app opens, before there is a slide, blurred and darkened like the background behind the slides. ' +
                    keyName(keyOf('setBackground', 0)) + ' uses the image being shown.</small></span>' +
                    '<div class="background-picker">' +
                        '<div id="background-preview">' + (settings.backgroundImage ? '' : 'None') + '</div>' +
                        '<button id="choose-background">Choose image…</button>' +
                        (settings.backgroundImage ? '<button id="remove-background">Remove</button>' : '') +
                    '</div></li>' +
                '<li><label class="row"><span>Random favorite at every start<small>Uses a different downloaded favorite (Settings → Favorites → Download favorites automatically) as the background each time the app opens. Without downloaded favorites, the image above is used.</small></span>' +
                    '<input type="checkbox" id="background-random-favorite"' + (settings.backgroundRandomFavorite ? ' checked' : '') + '></label></li>' +
                '<li><label class="row"><span>Include GIFs<small>In the random pick. Otherwise only still images.</small></span>' +
                    '<input type="checkbox" id="background-random-gifs"' + (settings.backgroundRandomGifs ? ' checked' : '') + '></label></li>' +
                '<li><label class="row"><span>Include videos<small>In the random pick (.webm and .mp4), played muted in a loop.</small></span>' +
                    '<input type="checkbox" id="background-random-videos"' + (settings.backgroundRandomVideos ? ' checked' : '') + '></label></li>' +
                sliderRow('background-blur', 'Blur', 'How blurred the background image is. 0 is sharp.', settings.backgroundBlur, 120) +
            '</ul>';

        setupBlurSlider(section, 'glass-blur', 'glassBlur', 'Glass blur', applyGlassBlur, false);
        setupBlurSlider(section, 'refract-blur', 'refractBlur', 'Refract blur', applyRefract, false);
        setupBlurSlider(section, 'refract-strength', 'refractStrength', 'Refract bending', applyRefract, false, ' %');
        setupBlurSlider(section, 'refract-ripple', 'refractRipple', 'Refract ripple', applyRefract, false, ' %');
        setupBlurSlider(section, 'refract-corner', 'refractCorner', 'Refract corners', applyRefract, false, ' %');
        setupBlurSlider(section, 'refract-tint', 'refractTint', 'Refract tint', applyRefract, false, ' %');
        setupBlurSlider(section, 'background-blur', 'backgroundBlur', 'Background blur', applyBackgroundBlur, true);

        section.querySelector('#search-sort-menu').addEventListener('change', function (e) {
            settings.searchSortMenu = e.target.checked;
            save('searchSortMenu');
            updateSearchSortMenu();
        });

        let cardsToggle = section.querySelector('#quick-cards-toggle');
        if (cardsToggle) {
            cardsToggle.addEventListener('change', function (e) {
                settings.quickCards = e.target.checked;
                save('quickCards');
                renderQuickCards();
            });
        }

        section.querySelector('#show-download-button').addEventListener('change', function (e) {
            settings.showDownloadButton = e.target.checked;
            save('showDownloadButton');
            updateDownloadButton();
        });

        section.querySelector('#app-show-tips').addEventListener('change', function (e) {
            settings.showTips = e.target.checked;
            save('showTips');
        });

        paintBackground(section.querySelector('#background-preview'), settings.backgroundImage);

        ['Favorite', 'Gifs', 'Videos'].forEach(function (name) {
            section.querySelector('#background-random-' + name.toLowerCase()).addEventListener('change', function (e) {
                settings['backgroundRandom' + name] = e.target.checked;
                save('backgroundRandom' + name);
                applyRandomBackground(section);
            });
        });

        section.querySelector('#choose-background').addEventListener('click', async function () {
            let url = await window.appInfo.chooseBackground();
            if (!url)
                return;
            setBackgroundImage(url);
        });

        let removeButton = section.querySelector('#remove-background');
        if (removeButton) {
            removeButton.addEventListener('click', async function () {
                await window.appInfo.clearBackground();
                setBackgroundImage('');
            });
        }

        section.querySelectorAll('input[name="app-theme"]').forEach(function (radio) {
            radio.addEventListener('change', function () {
                settings.appTheme = radio.value;
                save('appTheme');
                applyTheme();
            });
        });

        section.querySelectorAll('input[name="effects-style"]').forEach(function (radio) {
            radio.addEventListener('change', function () {
                settings.effectsStyle = radio.value;
                save('effectsStyle');
                applyLayoutSettings();
            });
        });

        section.querySelectorAll('input[name="tag-click"]').forEach(function (radio) {
            radio.addEventListener('change', function () {
                settings.tagClick = radio.value;
                save('tagClick');
            });
        });

        section.querySelectorAll('input[name="tags-position"]').forEach(function (radio) {
            radio.addEventListener('change', function () {
                settings.tagsPosition = radio.value;
                save('tagsPosition');
                applyLayoutSettings();
            });
        });

        section.querySelector('#app-show-tags').addEventListener('change', function (e) {
            setShowTags(e.target.checked);
        });

        section.querySelector('#app-click-opens-post')?.addEventListener('change', function (e) {
            settings.clickOpensPost = e.target.checked;
            save('clickOpensPost');
        });
    }

    function applyRefract() {
        window.LiquidGlass?.update({
            blur: settings.refractBlur,
            strength: settings.refractStrength / 100,
            ripple: settings.refractRipple / 100,
            corner: settings.refractCorner / 100,
            tint: settings.refractTint / 100
        });
    }

    function applyBackgroundBlur() {
        document.documentElement.style.setProperty('--background-blur', settings.backgroundBlur + 'px');
    }

    function applyGlassBlur() {
        document.documentElement.style.setProperty('--glass-blur', settings.glassBlur + 'px');
    }

    function setBackgroundImage(url) {
        settings.backgroundImage = url;
        save('backgroundImage');
        renderAppearance();
        updateForCurrentSlide();
    }

    function renderTouch() {
        let section = document.querySelector('.settings-section[data-section="touch"]');

        section.innerHTML =
            '<ul class="settings-list">' +
                '<li><label class="row"><span>Touch screen mode<small>Larger buttons, and tap gestures on the image.</small></span>' +
                    '<input type="checkbox" id="app-touch-mode"' + (settings.touchMode ? ' checked' : '') + '></label></li>' +
            '</ul>' +
            '<div class="touch-guide">' +
                '<div><b>Tap left edge</b><span>Previous</span></div>' +
                '<div><b>Tap middle</b><span>Controls / back to the image</span></div>' +
                '<div><b>Tap right edge</b><span>Next</span></div>' +
            '</div>' +
            '<p class="section-intro">' + (isFavoritesPage
                ? 'Double-tapping faves an image on the slideshow page. Here everything is already a favorite, so double-tap does nothing.'
                : '<b>Double-tap</b> anywhere on the image to fave or unfave it.') + '</p>';

        section.querySelector('#app-touch-mode').addEventListener('change', function (e) {
            settings.touchMode = e.target.checked;
            save('touchMode');
            applyLayoutSettings();
        });
    }

    function renderHotkeys(message) {
        let section = document.querySelector('.settings-section[data-section="hotkeys"]');

        let rows = HOTKEY_ACTIONS.map(function (action) {
            let buttons = '';
            for (let slot = 0; slot < action.slots; slot++)
                buttons += '<button class="key-button" data-action="' + action.id + '" data-slot="' + slot + '">' + keyName(keyOf(action.id, slot)) + '</button>';
            return '<li class="row"><span>' + action.label + (action.note ? '<small>' + action.note + '</small>' : '') + '</span><div class="key-buttons">' + buttons + '</div></li>';
        }).join('');

        section.innerHTML =
            '<p class="section-intro">Click a key, then press the new key or combination. Esc cancels, Backspace clears it. ' +
                'From "Show / hide tags" down, combinations like Ctrl+F work too; the ones above take single keys.</p>' +
            '<p class="hotkey-message" ' + (message ? '' : 'hidden') + '>' + (message || '') + '</p>' +
            '<ul class="settings-list">' + rows + '</ul>' +
            '<div class="section-footer"><button id="reset-hotkeys">Reset to defaults</button></div>' +
            '<p class="section-intro">Fixed keys: <b>F11</b> full screen, <b>Enter</b> play/pause and search, <b>Esc</b> close this window or leave full screen, <b>Ctrl+R</b> reload, <b>Ctrl+Shift+I</b> developer tools.</p>';

        section.querySelectorAll('.key-button').forEach(function (button) {
            button.addEventListener('click', function () {
                startRecordingHotkey(button);
            });
        });

        section.querySelector('#reset-hotkeys').addEventListener('click', function () {
            settings.appHotkeys = defaultHotkeys();
            save('appHotkeys');
            applyHotkeys();
            renderHotkeys('Hotkeys reset to the defaults.');
            renderAppearance();
        });
    }

    function renderFavoritesSection() {
        let section = document.querySelector('.settings-section[data-section="favorites"]');

        section.innerHTML =
            '<ul class="settings-list">' +
                '<li><label class="row"><span>Also fave on e621<small>Faving or unfaving an e621 image here does the same on your e621 account. Needs your e621 username and API key under Sites &amp; accounts.</small></span>' +
                    '<input type="checkbox" id="sync-e621-favorites"' + (settings.syncE621Favorites ? ' checked' : '') + '></label></li>' +
            '</ul>' +
            '<h4 class="subheading">Import</h4>' +
            '<p class="section-intro">Adds your favorites from the sites to the favorites list. It uses the login details under ' +
                (isFavoritesPage ? 'Sites &amp; accounts on the slideshow page\'s settings' : 'Sites &amp; accounts') + ':</p>' +
            '<ul class="settings-list">' +
                '<li class="row"><span>e621<small>Username (API key optional)</small></span></li>' +
                '<li class="row"><span>Derpibooru<small>API key</small></span></li>' +
                '<li class="row"><span>Gelbooru<small>User ID and API key</small></span></li>' +
            '</ul>' +
            '<p class="section-intro">Favorites already in the list are skipped, so this can be run again to pick up new ones.</p>' +
            '<div class="section-footer"><button id="import-favorites-button" class="primary">&#8615; Import site favorites</button></div>' +
            '<p id="import-favorites-status" class="hotkey-message" hidden></p>' +
            (isFavoritesPage
                ? '<h4 class="subheading">Download</h4>' +
                  '<p class="section-intro">Saves the favorites shown on the page (all, filtered or random) to the "favorites" folder in the download folder, 3 at a time. Files already there are skipped, so a cancelled download can be resumed.</p>' +
                  '<div class="section-footer"><button id="download-favorites-button" class="primary">&#128190; Download all favorites</button></div>' +
                  '<p id="download-favorites-status" class="hotkey-message" hidden></p>'
                : '') +
            '<h4 class="subheading">Test</h4>' +
            '<ul class="settings-list">' +
                '<li><label class="row"><span>joi.how (test)<small>Adds a ♥ joi button to the search bar and the favorites page: it opens the settings of <a href="https://github.com/clynamic/joi.how" target="_blank">joi.how</a> ' +
                    '(clynamic, GPL-3.0) with the search results loaded so far, or the favorites being shown (all, filtered or random), as its images. Begin starts the game there.</small></span>' +
                    '<input type="checkbox" id="joi-how"' + (settings.joiHow ? ' checked' : '') + '></label></li>' +
            '</ul>';

        setupSyncToggles();

        section.querySelector('#joi-how').addEventListener('change', function (e) {
            settings.joiHow = e.target.checked;
            save('joiHow');
            updateJoiButton();
        });

        let downloadButton = section.querySelector('#download-favorites-button');
        if (downloadButton) {
            let downloadStatus = section.querySelector('#download-favorites-status');
            downloadButton.addEventListener('click', async function () {
                let running = controller().isDownloadingFavorites;
                downloadStatus.hidden = false;
                downloadButton.innerHTML = '&#10005; Cancel download';
                await controller().downloadFavoritesButtonClicked(message => downloadStatus.textContent = message);
                if (!running)
                    downloadButton.innerHTML = '&#128190; Download all favorites';
            });
        }

        let button = section.querySelector('#import-favorites-button');
        let status = section.querySelector('#import-favorites-status');

        button.addEventListener('click', async function () {
            let model = controller()._model;

            button.disabled = true;
            status.hidden = false;

            try {
                let results = await new FavoritesImporter(model.personalList).importFavorites(function (message) {
                    status.textContent = message;
                });

                if (isFavoritesPage) {
                    model.saveImportedFavorites();
                } else {
                    model.dataLoader.savePersonalList();
                    model.favoriteButtonUpdatedEvent.notify();
                }

                status.textContent = 'Imported. ' + results.map(r => r.name + ': ' + r.added + ' added (' + r.found + ' found)').join(', ') + '.';
            } catch (e) {
                status.textContent = 'Importing failed: ' + e.message;
            }

            button.disabled = false;
        });
    }

    // ---------- Download button ----------

    // Only something from the internet can be downloaded: not the front page, not a file from disk.
    function updateDownloadButton() {
        let slide = hasSlide() ? currentSlide() : null;
        document.getElementById('download-button').hidden = !settings.showDownloadButton || !slide || !/^https?:/i.test(slide.fileUrl);
    }

    // Saves the image or video on the screen, like the download hotkey.
    function setupDownloadButton() {
        let button = document.getElementById('download-button');

        button.addEventListener('click', function () {
            button.blur();
            if (!currentSlide())
                return;
            controller()._view.downloadCurrentSlide();
            showToast('Saving to the download folder…');
        });

        updateDownloadButton();
    }

    // ---------- joi.how (test) ----------

    function updateJoiButton() {
        let button = document.getElementById('joi-button');
        if (button)
            button.hidden = !settings.joiHow;
    }

    // The search results loaded so far, or the favorites being shown, in joi.how's image format.
    function setupJoiButton() {
        let button = document.getElementById('joi-button');

        const types = { [MEDIA_TYPE_VIDEO]: 'video', [MEDIA_TYPE_GIF]: 'gif' };

        let panel = document.getElementById('joi-panel');
        let frame = document.getElementById('joi-frame');

        let close = function () {
            panel.hidden = true;
            frame.src = 'about:blank'; // stops the game and its sounds
        };

        document.getElementById('joi-close').addEventListener('click', close);
        // Esc inside joi.how (js in the joi build, msg.ts) asks to close.
        window.addEventListener('message', function (e) {
            if (e.source === frame.contentWindow && e.data === 'msg-joi-close')
                close();
        });
        document.addEventListener('keydown', function (e) {
            if (e.key == 'Escape' && !panel.hidden) {
                e.stopImmediatePropagation();
                close();
            }
        }, true);

        button.addEventListener('click', function () {
            button.blur();
            let model = controller()._model;
            let what = isFavoritesPage ? 'favorites' : 'results';
            let items = isFavoritesPage
                ? (model.filtered ? model.filteredPersonalList : model.personalList).personalListItems
                : model.sitesManager.allSortedSlides;
            let images = items.filter(item => item.fileUrl).map(item => ({
                thumbnail: item.previewFileUrl || item.fileUrl,
                preview: item.fileUrl,
                full: item.fileUrl,
                type: types[item.mediaType] || 'image',
                source: item.viewableWebsitePostUrl,
                id: item.siteId + '-' + item.id
            }));

            if (images.length == 0)
                return showToast(isFavoritesPage ? 'No favorites to play.' : 'Search for something first.');

            // joi.how keeps its image list in localStorage, which it shares with this page (both are app files).
            try {
                localStorage.setItem('images', JSON.stringify(images));
            } catch (e) {
                return showToast('joi.how: too many ' + what + ' (' + images.length + ').');
            }

            // The slideshow and its video stop while joi.how plays.
            if (model.isPlaying)
                document.getElementById('pause-button').click();
            document.getElementById('current-video').pause();

            frame.src = 'joi/index.html#/';
            panel.hidden = false;
            frame.focus();
            showToast('joi.how: ' + images.length + ' ' + what);
        });

        updateJoiButton();
    }

    // ---------- Data usage: offline copies and offline mode ----------

    function renderDataSection() {
        let section = document.querySelector('.settings-section[data-section="data"]');

        section.innerHTML =
            '<ul class="settings-list">' +
                '<li><label class="row"><span>Offline mode<small>Nothing is loaded from the internet. Search shows your folders (Settings → Folders) and the downloaded favorites and pools, ' +
                    'the downloaded favorites keeping their tags; the favorites page shows the downloaded favorites. pool:&lt;number&gt; opens a downloaded pool.</small></span>' +
                    '<input type="checkbox" id="offline-mode"' + (settings.offlineMode ? ' checked' : '') + '></label></li>' +
            '</ul>' +
            '<h4 class="subheading">Offline copies</h4>' +
            '<ul class="settings-list">' +
                '<li><label class="row"><span>Download favorites automatically<small>Keeps a copy of every favorite in the "favorites" folder of your download folder (Settings → Folders), new ones too. In the background; files already there are skipped.</small></span>' +
                    '<input type="checkbox" id="auto-download-favorites"' + (settings.autoDownloadFavorites ? ' checked' : '') + '></label></li>' +
                '<li><label class="row"><span>Download saved pools automatically<small>Every saved pool into "pools\\&lt;name&gt; (&lt;number&gt;)", the pages numbered in reading order.</small></span>' +
                    '<input type="checkbox" id="auto-download-pools"' + (settings.autoDownloadPools ? ' checked' : '') + '></label></li>' +
                '<li><label class="row"><span>Use downloaded copies<small>Shows images and videos from the disk when they have been downloaded, instead of loading them from the site again.</small></span>' +
                    '<input type="checkbox" id="use-local-copies"' + (settings.useLocalCopies ? ' checked' : '') + '></label></li>' +
                '<li class="row"><span class="muted" id="auto-download-status">Not running.</span></li>' +
            '</ul>';

        for (let [id, key] of [['auto-download-favorites', 'autoDownloadFavorites'], ['auto-download-pools', 'autoDownloadPools'], ['use-local-copies', 'useLocalCopies']]) {
            section.querySelector('#' + id).addEventListener('change', function (e) {
                settings[key] = e.target.checked;
                save(key);
                if (key == 'useLocalCopies')
                    loadLocalCopies();
            });
        }

        // Everything (sites, search, the favorites list) starts differently offline: the page reloads.
        section.querySelector('#offline-mode').addEventListener('change', async function (e) {
            settings.offlineMode = e.target.checked;
            await chrome.storage.sync.set({ offlineMode: settings.offlineMode });
            location.reload();
        });

        let downloadStatus = section.querySelector('#auto-download-status');
        window.appInfo.autoDownloadStatus().then(text => { if (text) downloadStatus.textContent = text; });
        window.appInfo.onAutoDownloadStatus(text => downloadStatus.textContent = text);
    }

    let copiesLoaded = false;

    function showOfflineFavorites() {
        controller().filterButtonClicked();
        if (localCopies.size == 0)
            showToast('Offline: none of the favorites are downloaded to ' + (settings.downloadFolder || 'Downloads\\MSG') + ' yet.');
    }

    async function loadLocalCopies() {
        let copies = {};
        try {
            if (settings.useLocalCopies || offlineMode)
                copies = await window.appInfo.listLocalCopies();
        } catch (e) {} // no download folder yet
        localCopies = new Map(Object.entries(copies));
        localImageStems = new Map([...localCopies].filter(([name]) => !/\.(webm|mp4)$/i.test(name)).map(([name, url]) => [name.replace(/\.[^.]+$/, ''), url]));

        copiesLoaded = true;

        // Offline, the favorites page shows only the downloaded favorites.
        if (offlineMode && isFavoritesPage && controller()._model.personalList.personalListItems.length)
            showOfflineFavorites();

        // The first slide may already be showing from the site.
        for (let id of ['current-image', 'current-video']) {
            let element = document.getElementById(id);
            let src = element.getAttribute('src');
            if (src && displayUrl(src) != src)
                element.src = displayUrl(src);
        }
    }

    // ---------- Tag analysis ----------
    // Images added to it (C or the 🧪 button) are compared: the tags most of them share can be searched for.

    // Tags that say nothing about what is in the image.
    const GENERIC_TAG = /^(hi_?res|absurd_?res|highres|absurdres|lowres|low_res|\d{4}|\d+:\d+|digital_media_\(artwork\)|[a-z]+_\(artwork\)|animated|webm|sound|comic|english_text|text|signature|watermark|url|patreon.*|conditional_dnp|dialogue)$/i;

    function analysisTagsOf(slide) {
        let groups = slide.tagGroups;
        let tags = groups && typeof groups == 'object'
            ? Object.keys(groups).filter(group => !['meta', 'invalid', 'lore'].includes(group)).flatMap(group => groups[group])
            : String(slide.tags || '').split(/\s+/);
        return [...new Set(tags.filter(tag => tag && !GENERIC_TAG.test(tag)))];
    }

    function analysisKey(slide) {
        return slide.siteId + '-' + slide.id;
    }

    function toggleCurrentInAnalysis() {
        let slide = hasSlide() ? currentSlide() : null;
        if (!slide)
            return;

        let key = analysisKey(slide);
        if (settings.analysisItems.some(item => item.key == key)) {
            settings.analysisItems = settings.analysisItems.filter(item => item.key != key);
            showToast('Removed from the tag analysis.');
        } else {
            settings.analysisItems = settings.analysisItems.concat({ key: key, tags: analysisTagsOf(slide).join(' '), thumb: slide.previewFileUrl || slide.fileUrl }).slice(-100);
            showToast('Added to the tag analysis (' + settings.analysisItems.length + ').');
        }
        save('analysisItems');
        updateAnalysisButtons();
    }

    function updateAnalysisButtons() {
        let slide = hasSlide() ? currentSlide() : null;
        let add = document.getElementById('analysis-add-button');
        if (add)
            add.classList.toggle('active', !!slide && settings.analysisItems.some(item => item.key == analysisKey(slide)));

        let open = document.getElementById('analysis-button');
        if (open) {
            open.hidden = settings.analysisItems.length == 0;
            open.innerHTML = '&#129514; Analyze (' + settings.analysisItems.length + ')';
        }
    }

    // The most shared tags first; ties go to the rarer tag name alphabetically.
    function analysisTopTags() {
        let counts = new Map();
        for (let item of settings.analysisItems)
            for (let tag of item.tags.split(' ').filter(t => t))
                counts.set(tag, (counts.get(tag) || 0) + 1);
        return [...counts].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).slice(0, 20);
    }

    function setupTagAnalysis() {
        // 🧪 in the navigation pill (after ♥), and "Analyze (n)" in the search pill.
        let add = document.createElement('button');
        add.className = 'nav-button';
        add.id = 'analysis-add-button';
        add.title = 'Add to / remove from the tag analysis (' + keyName(keyOf('addToAnalysis', 0)) + ')';
        add.innerHTML = '&#129514;';
        add.addEventListener('click', () => { add.blur(); toggleCurrentInAnalysis(); });
        let favoriteButton = document.getElementById('favorite-button'); // not on the favorites page
        if (favoriteButton)
            favoriteButton.after(add);
        else
            document.getElementById('navigation').prepend(add);

        let open = document.createElement('button');
        open.className = 'glass-button';
        open.id = 'analysis-button';
        open.hidden = true;
        open.title = 'The tags that the images added to the tag analysis share';
        document.getElementById('settings-button').before(open);

        let dialog = document.createElement('dialog');
        dialog.id = 'analysis-dialog';
        dialog.className = 'glass';
        dialog.innerHTML =
            '<header class="pools-header"><h3>Tag analysis <small id="analysis-count"></small></h3>' +
                '<select id="analysis-top" title="How many of the most shared tags are chosen"><option value="3">Top 3</option><option value="5">Top 5</option><option value="10">Top 10</option></select>' +
                '<button id="analysis-close" class="icon-button" title="Close (Esc)">&times;</button></header>' +
            '<div id="analysis-images"></div>' +
            '<p class="section-intro">Click a tag to choose it or leave it out. The number is how many of the images have it.</p>' +
            '<div id="analysis-tags"></div>' +
            '<footer class="analysis-footer">' +
                '<button id="analysis-clear" class="danger">Clear</button>' +
                (isFavoritesPage ? '<button id="analysis-filter">Filter favorites</button>' : '') +
                '<button id="analysis-search" class="primary">Search</button>' +
            '</footer>';
        document.body.appendChild(dialog);

        let chosen = new Set();

        let render = function (resetChoice) {
            let top = analysisTopTags();
            if (resetChoice)
                chosen = new Set(top.slice(0, settings.analysisTop).map(([tag]) => tag));

            dialog.querySelector('#analysis-count').textContent = settings.analysisItems.length + ' images';

            let images = dialog.querySelector('#analysis-images');
            images.textContent = '';
            for (let item of settings.analysisItems) {
                let tile = document.createElement('button');
                tile.className = 'analysis-image';
                tile.title = 'Remove from the analysis';
                let img = document.createElement('img');
                img.src = thumbnailUrl(item.thumb);
                img.alt = '';
                tile.appendChild(img);
                tile.addEventListener('click', () => {
                    settings.analysisItems = settings.analysisItems.filter(i => i.key != item.key);
                    save('analysisItems');
                    updateAnalysisButtons();
                    render(true);
                });
                images.appendChild(tile);
            }

            let tags = dialog.querySelector('#analysis-tags');
            tags.textContent = '';
            for (let [tag, count] of top) {
                let chip = document.createElement('button');
                chip.className = 'analysis-tag' + (chosen.has(tag) ? ' active' : '');
                chip.textContent = tag.replace(/_/g, ' ');
                let number = document.createElement('small');
                number.textContent = count + ' / ' + settings.analysisItems.length;
                chip.appendChild(number);
                chip.addEventListener('click', () => {
                    if (chosen.has(tag)) chosen.delete(tag); else chosen.add(tag);
                    render(false);
                });
                tags.appendChild(chip);
            }
            dialog.querySelector('#analysis-search').disabled = chosen.size == 0;
            let filter = dialog.querySelector('#analysis-filter');
            if (filter)
                filter.disabled = chosen.size == 0;
        };

        let query = () => [...chosen].join(' ');

        open.addEventListener('click', () => { open.blur(); render(true); dialog.showModal(); });
        dialog.querySelector('#analysis-close').addEventListener('click', () => dialog.close());

        let topMenu = dialog.querySelector('#analysis-top');
        topMenu.value = String(settings.analysisTop);
        topMenu.addEventListener('change', () => {
            settings.analysisTop = +topMenu.value;
            save('analysisTop');
            render(true);
        });

        dialog.querySelector('#analysis-clear').addEventListener('click', () => {
            settings.analysisItems = [];
            save('analysisItems');
            updateAnalysisButtons();
            dialog.close();
        });

        dialog.querySelector('#analysis-search').addEventListener('click', () => {
            dialog.close();
            if (isFavoritesPage) {
                location.href = 'slideshow.html#search=' + encodeURIComponent(query());
                return;
            }
            let searchText = document.getElementById('search-text');
            searchText.value = query();
            searchText.dispatchEvent(new CustomEvent('change'));
            document.getElementById('search-button').click();
        });

        let filterButton = dialog.querySelector('#analysis-filter');
        if (filterButton)
            filterButton.addEventListener('click', () => {
                dialog.close();
                searchForTag(query());
                document.getElementById('filter-button').click();
            });

        // Slideshow page opened with #search=<tags> (from the favorites page): search once a site has answered.
        let match = !isFavoritesPage && location.hash.match(/^#search=(.+)$/);
        if (match) {
            history.replaceState(null, '', location.pathname);
            let text = decodeURIComponent(match[1]);
            let tries = 0;
            let run = function () {
                let model = controller()._model;
                let ready = offlineMode || model.sitesManager.siteManagers.some(m => m.isOnline && model.sitesToSearch[m.id]);
                if (!ready && tries++ < 60)
                    return setTimeout(run, 250);
                let searchText = document.getElementById('search-text');
                searchText.value = text;
                searchText.dispatchEvent(new CustomEvent('change'));
                document.getElementById('search-button').click();
            };
            run();
        }

        updateAnalysisButtons();
    }

    // ---------- Sort menu in the search bar (Settings → Appearance) ----------

    const SORT_TERM = /(^|\s)(?:order|sort):\S+/gi;

    function setupSearchSortMenu() {
        let menu = document.getElementById('search-sort');
        if (!menu)
            return; // favorites page

        menu.value = settings.searchSort;
        menu.addEventListener('change', function () {
            menu.blur();
            settings.searchSort = menu.value;
            save('searchSort');
        });

        // The chosen order replaces any order: or sort: typed in the search.
        let model = controller()._model;
        let performSearch = model.performSearch;
        model.performSearch = function (searchText) {
            return performSearch.call(this, applySearchSort(searchText, this.getSelectedSitesToSearch(), false));
        };

        updateSearchSortMenu();
    }

    // The search text as it is sent: with the chosen order in place of a typed order: term.
    function applySearchSort(searchText, sites, quiet) {
        let sort = settings.searchSort;
        // Your folders have no scores: there the score orders mean newest first.
        let onlyFolders = offlineMode || sites.every(site => site == SITE_LOCAL);
        if (onlyFolders && /score/.test(sort)) {
            sort = 'order:id_desc';
            if (!quiet)
                showToast('Your folders have no scores, so they are sorted by newest.');
        }
        // Default leaves the search as it is typed; any other choice replaces a typed order: term.
        if (settings.searchSortMenu && sort && poolIdFromSearch(searchText) == null)
            searchText = searchText.replace(SORT_TERM, ' ').trim() + ' ' + sort;
        return searchText;
    }

    function updateSearchSortMenu() {
        let menu = document.getElementById('search-sort');
        if (menu)
            menu.hidden = !settings.searchSortMenu;
    }

    // ---------- e621 pools ----------

    // Slideshow page: "Save pool" while a pool is shown.
    function setupPoolSaving() {
        let button = document.getElementById('save-pool-button');
        if (!button)
            return; // favorites page

        let pool = null;

        let update = function () {
            let saved = pool && settings.savedPools.some(p => p.id == pool.id);
            button.hidden = !pool;
            button.innerHTML = saved ? '&#9733; Saved' : '&#9734; Save pool';
            button.classList.toggle('active', !!saved);
        };

        window.addEventListener('pool-loaded', function (e) {
            pool = e.detail;
            update();
            showToast('Pool: ' + pool.name + ' · ' + pool.count + ' pages');
        });

        // Another search (button or Enter) hides it until that one is a pool too.
        let searchStarted = function () {
            if (poolIdFromSearch(document.getElementById('search-text').value) == null) {
                pool = null;
                update();
            }
        };
        document.getElementById('search-button').addEventListener('click', searchStarted, true);
        document.getElementById('search-text').addEventListener('keydown', function (e) {
            if (e.key == 'Enter')
                searchStarted();
        });

        button.addEventListener('click', function () {
            button.blur();
            if (settings.savedPools.some(p => p.id == pool.id)) {
                settings.savedPools = settings.savedPools.filter(p => p.id != pool.id);
                showToast('Removed from Pools.');
            } else {
                let first = controller()._model.sitesManager.allSortedSlides[0];
                settings.savedPools = settings.savedPools.concat({ id: pool.id, name: pool.name, count: pool.count, cover: first ? first.previewFileUrl : '' });
                showToast('Saved to Pools.');
            }
            save('savedPools');
            update();
        });

    }

    // Searches for a pool, once e621 has answered its status check.
    function openPool(id) {
        let tries = 0;
        let open = function () {
            let e621 = controller()._model.sitesManager.siteManagers.find(m => m.id == SITE_E621);
            if (!(e621 && e621.isOnline) && tries++ < 60)
                return setTimeout(open, 250);
            let searchText = document.getElementById('search-text');
            searchText.value = 'pool:' + id;
            searchText.dispatchEvent(new CustomEvent('change'));
            document.getElementById('search-button').click();
        };
        open();
    }

    // Slideshow page: the saved pools as covers, in a window opened from the Pools button.
    function setupPoolsBrowser() {
        let dialog = document.getElementById('pools-dialog');
        if (!dialog)
            return; // favorites page

        let filter = document.getElementById('pools-filter');
        let status = document.getElementById('pools-status');
        let findButton = document.getElementById('find-pools-button');

        document.getElementById('pools-button').addEventListener('click', function () {
            this.blur();
            renderPools();
            dialog.showModal();
        });
        document.getElementById('pools-close').addEventListener('click', () => dialog.close());
        filter.addEventListener('input', renderPools);

        let sort = document.getElementById('pools-sort');
        sort.value = settings.poolsSort;
        sort.addEventListener('change', function () {
            settings.poolsSort = sort.value;
            save('poolsSort');
            renderPools();
        });

        findButton.addEventListener('click', async function () {
            findButton.disabled = true;
            status.hidden = false;
            try {
                let result = await findPoolsInFavorites(message => status.textContent = message);
                status.textContent = 'Checked ' + result.checked + ' e621 favorites: ' + result.found + ' pools, ' + result.added + ' new.';
            } catch (e) {
                status.textContent = 'Finding pools failed: ' + e.message;
            }
            findButton.disabled = false;
            renderPools();
        });
    }

    function renderPools() {
        let list = document.getElementById('pool-list');
        if (!list)
            return;

        let words = document.getElementById('pools-filter').value.toLowerCase().split(/\s+/).filter(w => w);
        let pools = settings.savedPools.filter(pool => words.every(w => pool.name.toLowerCase().includes(w)));

        if (settings.poolsSort == 'name')
            pools.sort((a, b) => a.name.localeCompare(b.name));
        else if (settings.poolsSort == 'pages')
            pools.sort((a, b) => b.count - a.count);
        else
            pools.reverse(); // newest saved first

        document.getElementById('pools-count').textContent = settings.savedPools.length || '';
        list.textContent = '';

        if (pools.length == 0) {
            let empty = document.createElement('p');
            empty.className = 'muted';
            empty.textContent = settings.savedPools.length
                ? 'No pools match.'
                : 'No pools yet. Search for pool:<number> or paste a pool link and press Save pool, or use Find pools in favorites.';
            list.appendChild(empty);
            return;
        }

        for (let pool of pools) {
            let tile = document.createElement('a');
            tile.className = 'pool-tile';
            tile.href = '#pool=' + pool.id;
            tile.title = pool.name;
            tile.addEventListener('click', function (e) {
                e.preventDefault();
                document.getElementById('pools-dialog').close();
                openPool(pool.id);
            });

            let cover = document.createElement('img');
            cover.src = thumbnailUrl(pool.cover || '');
            cover.alt = '';
            cover.loading = 'lazy';

            let name = document.createElement('span');
            name.className = 'pool-name';
            name.textContent = pool.name;

            let count = document.createElement('small');
            count.textContent = pool.count + ' pages';

            let remove = document.createElement('button');
            remove.className = 'pool-remove icon-button';
            remove.title = 'Remove from the saved pools';
            remove.innerHTML = '&times;';
            remove.addEventListener('click', function (e) {
                e.preventDefault();
                settings.savedPools = settings.savedPools.filter(p => p.id != pool.id);
                save('savedPools');
                renderPools();
            });

            tile.append(cover, name, count, remove);
            list.appendChild(tile);
        }
    }

    // The pools that the e621 favorites are in: the posts 100 at a time (their "pools"), then the new
    // pools' names and first pages. Read-only; a short pause between requests keeps within e621's rate limit.
    async function findPoolsInFavorites(onProgress) {
        let login = await chrome.storage.sync.get(['e621Login', 'e621ApiKey']);
        let headers = login.e621Login && login.e621ApiKey ? { Authorization: 'Basic ' + btoa(login.e621Login + ':' + login.e621ApiKey) } : {};

        let get = async function (path) {
            await new Promise(r => setTimeout(r, 600));
            let response = await fetch('https://e621.net' + path, { headers: headers });
            if (!response.ok)
                throw new Error('e621 answered ' + response.status);
            return response.json();
        };

        let chunks = (list, size) => Array.from({ length: Math.ceil(list.length / size) }, (_, i) => list.slice(i * size, i * size + size));

        let favorites = controller()._model.personalList.personalListItems.filter(item => item.siteId == SITE_E621);
        let poolIds = new Set();
        let checked = 0;

        for (let ids of chunks(favorites.map(item => item.id), 100)) {
            let data = await get('/posts.json?limit=100&tags=id:' + ids.join(','));
            for (let post of data.posts)
                for (let id of post.pools || [])
                    poolIds.add(id);
            checked += ids.length;
            onProgress('Checking favorites: ' + checked + ' / ' + favorites.length + ' · ' + poolIds.size + ' pools');
        }

        let newIds = [...poolIds].filter(id => !settings.savedPools.some(p => p.id == id));
        let added = [];

        for (let ids of chunks(newIds, 100)) {
            onProgress('Getting pool names: ' + added.length + ' / ' + newIds.length);
            let pools = await get('/pools.json?limit=100&search%5Bid%5D=' + ids.join(','));
            for (let pool of pools)
                added.push({ id: pool.id, name: String(pool.name).replace(/_/g, ' '), count: pool.post_count, first: (pool.post_ids || [])[0], cover: '' });
        }

        // Covers: each pool's first page.
        for (let ids of chunks([...new Set(added.map(p => p.first).filter(id => id))], 100)) {
            onProgress('Getting covers…');
            let data = await get('/posts.json?limit=100&tags=id:' + ids.join(','));
            let previews = new Map(data.posts.map(post => [post.id, post.preview && post.preview.url]));
            for (let pool of added)
                if (previews.get(pool.first))
                    pool.cover = previews.get(pool.first);
        }

        added.sort((a, b) => a.name.localeCompare(b.name));
        settings.savedPools = settings.savedPools.concat(added.map(({ first, ...pool }) => pool));
        save('savedPools');

        return { checked: favorites.length, found: poolIds.size, added: added.length };
    }

    // "Also fave on e621" in Settings → Favorites.
    function setupSyncToggles() {
        let toggle = document.getElementById('sync-e621-favorites');

        toggle.checked = settings.syncE621Favorites;
        toggle.addEventListener('change', function () {
            settings.syncE621Favorites = toggle.checked;
            save('syncE621Favorites');
        });
    }

    async function renderAbout() {
        let section = document.querySelector('.settings-section[data-section="about"]');
        let version = await window.appInfo.getVersion();

        section.innerHTML =
            '<div class="about-header">' +
                '<img class="logo" src="img/msg_logo.svg" alt="">' +
                '<div><h2>' + APP_NAME + '</h2><p class="muted">Version ' + version + ' · Electron ' + window.appInfo.electronVersion + ' · Chromium ' + window.appInfo.chromeVersion + '</p></div>' +
            '</div>' +
            '<p class="section-intro">MSG is monosodium glutamate, food additive E621. The logo is its structural formula.</p>' +

            '<h4 class="subheading">Based on</h4>' +
            '<div class="credit">' +
                '<p><b><a href="https://github.com/Chirmaya/BooruSlideshow" target="_blank">BooruSlideshow</a></b> by Chirmaya: the slideshow, site searches and favorites come from version 10.6 of this browser extension.</p>' +
                '<pre class="license-text">Copyright (c) 2026 Chirmaya\n\nOpen license to copy/modify/etc. as long as you:\n- Don\'t publish under the name "Booru Slideshow" (to reduce name confusion)</pre>' +
            '</div>' +
            '<div class="credit">' +
                '<p><b><a href="https://github.com/michutsu/BooruSlideshowElectron" target="_blank">BooruSlideshowElectron</a></b> by michutsu: the earlier Electron version of the extension, which this app replaces. It showed how to run the extension in Electron; no code from it is used.</p>' +
            '</div>' +

            '<h4 class="subheading">Built with</h4>' +
            '<div class="credit">' +
                '<p><b><a href="https://www.electronjs.org/" target="_blank">Electron</a></b>, MIT license, Copyright (c) Electron contributors, Copyright (c) 2013-2020 GitHub Inc. The full license is in the LICENSE file and Chromium\'s licenses in LICENSES.chromium.html, both next to the app.</p>' +
            '</div>' +

            '<div class="credit">' +
                '<p><b><a href="https://libraries.dev" target="_blank">Libraries.dev</a></b> by Jakub Antalik: the <a href="https://libraries.dev/beam" target="_blank">border beam</a> on the search bar (border-beam 1.4.1) and the <a href="https://libraries.dev/orbs" target="_blank">thinking orb</a> loading animation (thinking-orbs 0.3.2, made with Alexandr Brinza). MIT license, Copyright (c) 2026 Jakub Antalik. The full license is at the top of css/vendor/border-beam.css and js/vendor/thinking-orbs-engine.js.</p>' +
                '<p><b><a href="https://github.com/dashersw/liquid-glass-js" target="_blank">liquid-glass-js</a></b> by Armagan Amcalar: the glass shader of the Refract effects style (test). MIT license, Copyright (c) 2025 Armagan Amcalar. The full license is at the top of js/liquid_glass.js.</p>' +
                '<p><b><a href="https://github.com/clynamic/joi.how" target="_blank">joi.how</a></b> by clynamic (GPL-3.0): the ♥ joi button on the favorites page (test). A build of it is in the joi folder, with the changes made for MSG (joi/msg-changes.patch). GPL-3.0; the full license is in joi/LICENSE.</p>' +
            '</div>' +

            '<h4 class="subheading">Content</h4>' +
            '<p class="section-intro">Images, videos and tags come from the sites you search and belong to their artists and sites. The tag panel follows e621\'s tag categories and colors.</p>';

        setupDeveloperUnlock();
    }

    // ---------- Developer settings ----------

    // Five quick clicks on the logo in About reveal the Developer section, like Android's build number.
    function setupDeveloperUnlock() {
        let clicks = 0;
        let timer = null;

        document.querySelector('.about-header .logo').addEventListener('click', function () {
            clearTimeout(timer);
            timer = setTimeout(() => clicks = 0, 2000);
            if (++clicks < 5)
                return;

            clicks = 0;
            if (settings.developerMode)
                return showToast('Developer settings are already on.');

            settings.developerMode = true;
            save('developerMode');
            updateDeveloperTab();
            showToast('Developer settings unlocked: Settings → Developer.');
        });
    }

    // The report is refreshed twice a second, but only while the developer settings are on.
    let developerTimer = 0;

    function updateDeveloperTab() {
        document.querySelector('.settings-tab[data-section="developer"]').hidden = !settings.developerMode;

        if (settings.developerMode && !developerTimer) {
            developerTimer = setInterval(function () {
                updateFpsLoop();
                refreshDeveloper();
            }, 500);
        } else if (!settings.developerMode && developerTimer) {
            clearInterval(developerTimer);
            developerTimer = 0;
        }
    }

    function formatBytes(bytes) {
        return bytes >= 1048576 ? (bytes / 1048576).toFixed(2) + ' MB' : Math.round(bytes / 1024) + ' kB';
    }

    // The size of a file from its Content-Length, fetched once per file, and never offline.
    // ponytail: files on the disk show —; add a stat call in main.js if their size is wanted.
    const fileSizes = new Map();

    function fileSize(url) {
        if (!fileSizes.has(url)) {
            fileSizes.set(url, '…');
            if (offlineMode || !/^https?:/i.test(url))
                fileSizes.set(url, '—');
            else
                fetch(url, { method: 'HEAD' })
                    .then(r => fileSizes.set(url, Number(r.headers.get('content-length')) ? formatBytes(Number(r.headers.get('content-length'))) : '—'))
                    .catch(() => fileSizes.set(url, '—'));
        }
        return fileSizes.get(url);
    }

    // Frames per second of the page and of the video on the screen, from a requestAnimationFrame
    // loop that only runs while something shows the numbers.
    const fps = { raf: 0, frames: 0, last: 0, page: null, video: null, videoFrames: 0, videoDropped: 0 };

    function fpsLoop(now) {
        fps.frames++;

        if (now - fps.last >= 1000) {
            let seconds = (now - fps.last) / 1000;
            let video = document.getElementById('current-video');
            let quality = video && video.readyState > 0 && !video.paused && video.getVideoPlaybackQuality();

            fps.page = Math.round(fps.frames / seconds);
            fps.video = null;
            // The first second has nothing to compare with, and a new video starts its counters again.
            if (quality && fps.videoFrames != null && quality.totalVideoFrames >= fps.videoFrames)
                fps.video = { fps: Math.round((quality.totalVideoFrames - fps.videoFrames) / seconds), dropped: quality.droppedVideoFrames - fps.videoDropped };
            fps.videoFrames = quality ? quality.totalVideoFrames : null;
            fps.videoDropped = quality ? quality.droppedVideoFrames : 0;
            fps.frames = 0;
            fps.last = now;
            updateHud();
        }

        fps.raf = requestAnimationFrame(fpsLoop);
    }

    function updateFpsLoop() {
        let wanted = settings.showFps || developerVisible();

        if (wanted && !fps.raf) {
            fps.frames = 0;
            fps.videoFrames = null;
            fps.last = performance.now();
            fps.raf = requestAnimationFrame(fpsLoop);
        } else if (!wanted && fps.raf) {
            cancelAnimationFrame(fps.raf);
            fps.raf = 0;
            fps.page = fps.video = null;
        }
    }

    function fpsText() {
        if (fps.page == null)
            return 'FPS …';
        return 'FPS ' + fps.page + (fps.video ? ' · video ' + fps.video.fps + ' fps, ' + fps.video.dropped + ' dropped' : '');
    }

    // The box in the top right corner: FPS and the background download's progress.
    let downloadText = '';
    let downloadClearTimer = null;

    function setDownloadText(text) {
        downloadText = text || '';
        clearTimeout(downloadClearTimer);
        // Progress reads "favorites: 120 / 4000"; anything else is a result that goes away.
        if (!/ \/ \d+/.test(downloadText))
            downloadClearTimer = setTimeout(function () {
                downloadText = '';
                updateHud();
            }, 8000);
        updateHud();
    }

    // A single download (the button or L): "Downloading name: 45%", then a result that goes away.
    let singleText = '';
    let singleClearTimer = null;

    function setSingleText(text) {
        singleText = text || '';
        clearTimeout(singleClearTimer);
        if (!/^Downloading /.test(singleText))
            singleClearTimer = setTimeout(function () {
                singleText = '';
                updateHud();
            }, 8000);
        updateHud();
    }

    function updateHud() {
        let hud = document.getElementById('dev-hud');

        if (!hud) {
            hud = document.createElement('div');
            hud.id = 'dev-hud';
            hud.className = 'glass';
            hud.hidden = true;
            document.body.appendChild(hud);
        }

        let lines = [];
        if (settings.showFps)
            lines.push(fpsText());
        if (settings.showDownloadProgress && downloadText)
            lines.push(downloadText);
        if (settings.showDownloadProgress && singleText)
            lines.push(singleText);

        hud.replaceChildren(...lines.map(function (line) {
            let div = document.createElement('div');
            div.textContent = line;
            return div;
        }));
        hud.hidden = lines.length == 0;
    }

    function developerVisible() {
        let dialog = document.getElementById('settings-dialog');
        return settings.developerMode && dialog.open && currentSettingsSection == 'developer' && !dialog.querySelector('.settings-content.searching');
    }

    let appVersion = '';

    // The report: groups of [label, value]. Site data goes in as text only.
    function buildReport() {
        let model = controller() && controller()._model;
        let slide = currentSlide();
        let image = document.getElementById('current-image');
        let video = document.getElementById('current-video');
        let size = (w, h) => w && h ? w + ' × ' + h : '—';
        let seconds = s => Number.isFinite(s) ? s.toFixed(1) + ' s' : '—';
        let safe = fn => { try { return fn(); } catch (e) { return undefined; } };
        let groups = [];

        if (!slide) {
            groups.push({ title: 'Slide', rows: [['Slide', 'none on the screen']] });
        } else {
            let url = slide.fileUrl || '';
            let shown = displayUrl(url);
            let name = url.split(/[?#]/)[0].split('/').pop();
            let extension = (name.match(/\.(\w+)$/) || [])[1];
            let isVideo = slide.isVideo();
            let element = isVideo ? video : image;
            let site = slide.siteId == SITE_LOCAL ? 'Your folders' :
                safe(() => model.sitesManager.siteManagers.find(m => m.id == slide.siteId).url.replace(/^https?:\/\//, '')) || slide.siteId;

            try {
                name = decodeURIComponent(name);
            } catch (e) {}

            let rows = [
                ['Site', site],
                ['Post id', slide.id],
                ['File name', name],
                ['Format', (extension ? extension.toUpperCase() : '?') + ' (' + slide.mediaType + ')'],
                ['File size', fileSize(shown)],
                ['Loaded from', /^file:/i.test(shown) ? 'disk' : 'internet'],
                ['Source resolution', size(slide.width, slide.height)],
                ['Frame, natural', isVideo ? size(video.videoWidth, video.videoHeight) : size(image.naturalWidth, image.naturalHeight)],
                ['Frame, displayed', size(element.clientWidth, element.clientHeight)],
                ['MD5', slide.md5 || '—'],
                // e621's score is {up, down, total}, and the date a Date.
                ['Score', (slide.score && slide.score.total != null ? slide.score.total : slide.score) ?? '—'],
                ['Date', slide.date instanceof Date ? slide.date.toLocaleString() : slide.date || '—'],
                ['Tags', slide.tags ? String(slide.tags).split(/\s+/).filter(Boolean).length : 0]
            ];

            if (isVideo) {
                let quality = safe(() => video.getVideoPlaybackQuality());
                rows.push(
                    ['Video position', seconds(video.currentTime) + ' / ' + seconds(video.duration)],
                    ['Video state', (video.paused ? 'paused' : 'playing') + (video.muted ? ', muted' : '') + ', volume ' + Math.round(video.volume * 100) + ' %, ready state ' + video.readyState],
                    ['Video frames', quality ? quality.totalVideoFrames + ' shown, ' + quality.droppedVideoFrames + ' dropped' : '—']
                );
            }

            groups.push({ title: 'Slide', rows });
        }

        groups.push({
            title: 'Performance',
            rows: [
                ['Page FPS', fps.page != null ? fps.page : '…'],
                ['Video FPS', fps.video ? fps.video.fps + ' (' + fps.video.dropped + ' dropped)' : '—'],
                ['JS heap', performance.memory ? Math.round(performance.memory.usedJSHeapSize / 1048576) + ' MB' : '—']
            ]
        });

        let slideCount = safe(() => model.getSlideCount());
        let enabled = safe(() => model.sitesManager.siteManagers.filter(m => m.isEnabled).map(m => m.id == SITE_LOCAL ? 'folders' : m.url.replace(/^https?:\/\//, '')).join(', '));

        groups.push({
            title: 'Window',
            rows: [
                ['Window, inner', size(innerWidth, innerHeight)],
                ['Window, outer', size(outerWidth, outerHeight)],
                ['Screen', size(screen.width, screen.height)],
                ['Pixel ratio', devicePixelRatio],
                ['Fullscreen', innerWidth == screen.width && innerHeight == screen.height ? 'yes' : 'no'],
                ['Theme', document.documentElement.dataset.theme + ', effects ' + settings.effectsStyle],
                ['Touch mode', settings.touchMode ? 'on' : 'off'],
                ['Auto-fit', safe(() => model.autoFitSlide) ? 'on' : 'off, max ' + safe(() => size(model.maxWidth, model.maxHeight))]
            ]
        });

        groups.push({
            title: 'Session',
            rows: [
                ['Page', isFavoritesPage ? 'Favorites' : 'Slideshow'],
                ['Slide', slideCount != null ? safe(() => model.getCurrentSlideNumber()) + ' / ' + slideCount : '—'],
                ['Sites', enabled || '—'],
                ['Offline mode', offlineMode ? 'on' : 'off'],
                ['Downloaded copies', localCopies.size],
                ['MSG', appVersion + ' · Electron ' + window.appInfo.electronVersion + ' · Chromium ' + window.appInfo.chromeVersion],
                ['Platform', navigator.platform]
            ]
        });

        return groups;
    }

    let reportSignature = '';
    let reportText = '';

    // Updates the values in place, so text selected in the report stays selected.
    function refreshDeveloper() {
        if (!developerVisible())
            return;

        let groups = buildReport();
        let box = document.getElementById('dev-report');
        let signature = groups.map(g => g.title + g.rows.map(r => r[0]).join()).join('|');

        reportText = groups.map(g => g.title + '\n' + g.rows.map(r => r[0] + ': ' + r[1]).join('\n')).join('\n\n');

        if (signature != reportSignature) {
            reportSignature = signature;
            box.replaceChildren(...groups.flatMap(function (group) {
                let heading = document.createElement('h4');
                heading.className = 'subheading';
                heading.textContent = group.title;

                let table = document.createElement('table');
                table.className = 'dev-table';
                for (let row of group.rows) {
                    let tr = table.insertRow();
                    tr.insertCell().textContent = row[0];
                    tr.insertCell();
                }
                return [heading, table];
            }));
        }

        let cells = box.querySelectorAll('tr > td:last-child');
        groups.flatMap(g => g.rows).forEach(function (row, i) {
            if (cells[i].textContent != String(row[1]))
                cells[i].textContent = row[1];
        });
    }

    function renderDeveloper() {
        let section = document.querySelector('.settings-section[data-section="developer"]');

        section.innerHTML =
            '<p class="section-intro">Numbers for debugging and for watching performance. They update while this page is open.</p>' +
            '<ul class="settings-list">' +
                '<li><label class="row"><span>Show FPS<small>The frame rate of the page and of the video, in the top right corner.</small></span>' +
                    '<input type="checkbox" id="dev-fps"' + (settings.showFps ? ' checked' : '') + '></label></li>' +
                '<li><label class="row"><span>Show download progress<small>The background download of favorites and pools (Settings → Data usage) and downloads made with the button or the hotkey, in the top right corner.</small></span>' +
                    '<input type="checkbox" id="dev-downloads"' + (settings.showDownloadProgress ? ' checked' : '') + '></label></li>' +
                '<li><label class="row"><span>Log to the console<small>Slide addresses and other messages in the developer tools (F12). Until the app is restarted.</small></span>' +
                    '<input type="checkbox" id="dev-log"' + (LOGGING_MODE == LOGGING_MODE_DEV ? ' checked' : '') + '></label></li>' +
            '</ul>' +
            '<div id="dev-report"></div>' +
            '<p><button id="dev-copy">Copy the report</button> <button id="dev-hide" class="danger">Hide developer settings</button></p>';

        for (let [id, key] of [['dev-fps', 'showFps'], ['dev-downloads', 'showDownloadProgress']]) {
            section.querySelector('#' + id).addEventListener('change', function (e) {
                settings[key] = e.target.checked;
                save(key);
                updateHud();
                updateFpsLoop();
            });
        }

        section.querySelector('#dev-log').addEventListener('change', e => e.target.checked ? enableDevMode() : disableDevMode());

        section.querySelector('#dev-copy').addEventListener('click', function () {
            navigator.clipboard.writeText(reportText).then(() => showToast('Report copied.'), () => showToast('Copying failed.'));
        });

        section.querySelector('#dev-hide').addEventListener('click', function () {
            // The corner box goes too, so nothing is left on that can't be turned off.
            for (let key of ['developerMode', 'showFps', 'showDownloadProgress']) {
                settings[key] = false;
                save(key);
            }
            disableDevMode();
            for (let id of ['dev-fps', 'dev-downloads', 'dev-log'])
                section.querySelector('#' + id).checked = false;
            updateDeveloperTab();
            updateHud();
            updateFpsLoop();
            showSettingsSection('about');
        });

        window.appInfo.getVersion().then(v => appVersion = v);
        window.appInfo.autoDownloadStatus().then(setDownloadText);
        window.appInfo.onAutoDownloadStatus(setDownloadText);
        window.appInfo.onDownloadProgress(setSingleText);

        updateDeveloperTab();
        updateHud();
        updateFpsLoop();
        refreshDeveloper();
    }

    let currentSettingsSection = 'appearance';

    // Leaves the search: the rows come back and the search box is emptied.
    function stopSettingsSearch() {
        document.querySelector('.settings-content').classList.remove('searching');
        document.querySelectorAll('.search-miss').forEach(function (e) { e.classList.remove('search-miss'); });
        document.querySelector('.settings-no-match').hidden = true;
        document.getElementById('settings-search').value = '';
    }

    function showSettingsSection(name) {
        let dialog = document.getElementById('settings-dialog');
        let section = dialog.querySelector('.settings-section[data-section="' + name + '"]') || dialog.querySelector('.settings-section');
        name = section.dataset.section;
        currentSettingsSection = name;

        stopSettingsSearch();
        dialog.querySelectorAll('.settings-section').forEach(function (s) { s.hidden = s !== section; });
        dialog.querySelectorAll('.settings-tab').forEach(function (tab) { tab.classList.toggle('active', tab.dataset.section == name); });
        document.getElementById('settings-section-title').textContent = section.dataset.title;
        dialog.querySelector('.settings-scroll').scrollTop = 0;

        updateFpsLoop();
        refreshDeveloper();

        try {
            localStorage.setItem('settingsSection', name);
        } catch (e) {}
    }

    // Shows the rows of every section that have all the words typed in the search box.
    // A section or a subheading whose own title matches shows everything under it.
    function searchSettings() {
        let dialog = document.getElementById('settings-dialog');
        let words = document.getElementById('settings-search').value.toLowerCase().split(/\s+/).filter(Boolean);
        if (words.length == 0)
            return showSettingsSection(currentSettingsSection);

        let matches = text => words.every(word => text.toLowerCase().includes(word));
        let found = false;

        dialog.querySelector('.settings-content').classList.add('searching');
        document.getElementById('settings-section-title').textContent = 'Search results';
        dialog.querySelectorAll('.settings-tab').forEach(function (tab) { tab.classList.remove('active'); });

        dialog.querySelectorAll('.settings-section').forEach(function (section) {
            // The developer settings stay out of the results until they are unlocked.
            if (section.dataset.section == 'developer' && !settings.developerMode) {
                section.classList.add('search-miss');
                return;
            }

            let showAll = matches(section.dataset.title);
            let any = false;

            for (let child of section.children) {
                if (child.matches('h4.subheading'))
                    showAll = matches(section.dataset.title) || matches(child.textContent);

                let units = child.matches('.settings-list') ? Array.from(child.children) : [child];
                let shown = 0;
                for (let unit of units) {
                    let hide = !showAll && !matches(unit.textContent);
                    unit.classList.toggle('search-miss', hide);
                    shown += hide ? 0 : 1;
                }
                if (child.matches('.settings-list'))
                    child.classList.toggle('search-miss', shown == 0);
                any = any || shown > 0;
            }

            section.hidden = false;
            section.classList.toggle('search-miss', !any);
            found = found || any;
        });

        let notice = dialog.querySelector('.settings-no-match');
        notice.hidden = found;
        if (!found) {
            notice.textContent = 'No settings match "' + words.join(' ') + '".';
            if (isFavoritesPage) {
                let link = document.createElement('a');
                link.textContent = 'the slideshow page\'s settings';
                link.addEventListener('click', () => openSlideshowSettings('sites'));
                notice.append(' The search, filtering, quick search and history settings are in ', link, '.');
            }
        }
    }

    // The settings of the slideshow page, from the favorites page.
    function openSlideshowSettings(section) {
        if (!confirm('This leaves the favorites page and opens the slideshow page\'s settings.\n\nContinue?'))
            return;
        location.href = 'slideshow.html#settings=' + section;
    }

    function setupSettingsSearch() {
        let dialog = document.getElementById('settings-dialog');
        let input = document.getElementById('settings-search');

        let notice = document.createElement('p');
        notice.className = 'settings-no-match';
        notice.hidden = true;
        dialog.querySelector('.settings-scroll').appendChild(notice);

        input.addEventListener('input', searchSettings);

        // The first Esc only empties the search; the window closes with the next one.
        input.addEventListener('keydown', function (e) {
            if (e.key == 'Escape' && input.value) {
                e.preventDefault();
                e.stopPropagation();
                showSettingsSection(currentSettingsSection);
            }
        });

        // A section that draws itself again (e.g. the hotkeys) is searched again.
        let pending = false;
        // (The sections only, not the notice, which the search itself rewrites.)
        let observer = new MutationObserver(function () {
            if (!input.value || pending)
                return;
            pending = true;
            requestAnimationFrame(function () {
                pending = false;
                searchSettings();
            });
        });
        dialog.querySelectorAll('.settings-section').forEach(function (section) {
            observer.observe(section, { childList: true, subtree: true });
        });

        dialog.addEventListener('close', function () {
            if (input.value)
                showSettingsSection(currentSettingsSection);
        });
    }

    function setupSettingsWindow() {
        let dialog = document.getElementById('settings-dialog');
        let settingsButton = document.getElementById('settings-button');

        setupSettingsSearch();

        dialog.querySelectorAll('.settings-tab').forEach(function (tab) {
            tab.addEventListener('click', function () {
                if (tab.dataset.page)
                    openSlideshowSettings(tab.dataset.section);
                else
                    showSettingsSection(tab.dataset.section);
            });
        });

        let lastSection = null;
        try {
            lastSection = localStorage.getItem('settingsSection');
        } catch (e) {}
        showSettingsSection(lastSection == 'developer' && !settings.developerMode ? 'appearance' : lastSection || 'appearance');

        // Opened from the favorites page's settings with #settings=<section>.
        let requested = !isFavoritesPage && location.hash.match(/^#settings=(\w+)$/);
        if (requested) {
            history.replaceState(null, '', location.pathname);
            showSettingsSection(requested[1]);
            dialog.showModal();
        }

        settingsButton.addEventListener('click', function () {
            settingsButton.blur();
            dialog.showModal();
        });

        document.getElementById('settings-close-button').addEventListener('click', function () {
            dialog.close();
        });

        // Clicking the dimmed area outside the window closes it.
        dialog.addEventListener('click', function (e) {
            if (e.target === dialog)
                dialog.close();
        });

        dialog.addEventListener('close', stopRecordingHotkey);
    }

    // ---------- Start ----------

    document.addEventListener('DOMContentLoaded', async function () {
        let stored = await chrome.storage.sync.get(Object.keys(DEFAULTS));

        for (let key in DEFAULTS) {
            if (stored[key] != null)
                settings[key] = stored[key];
        }

        // Fill in actions added after the hotkeys were saved.
        let hotkeys = defaultHotkeys();
        for (let id in hotkeys) {
            if (Array.isArray(settings.appHotkeys[id]))
                hotkeys[id] = settings.appHotkeys[id];
        }
        settings.appHotkeys = hotkeys;

        offlineMode = settings.offlineMode;

        // Only for this run: the saved backgroundImage stays as it is.
        if (settings.backgroundRandomFavorite)
            settings.backgroundImage = await window.appInfo.randomFavoriteImage(settings.backgroundRandomGifs, settings.backgroundRandomVideos) || settings.backgroundImage;

        applyTheme();
        applyHotkeys();
        applyLayoutSettings();
        applyBackgroundBlur();
        applyGlassBlur();

        // Notices from other parts of the app, e.g. faving on e621 failing (remote_favorites.js).
        window.addEventListener('app-notice', function (e) {
            showToast(e.detail);
        });

        window.addEventListener('favorite-toggled', function (e) {
            showFavoriteHeart(e.detail.faved);
        });
        watchFavoriteState();

        renderAppearance();
        renderTouch();
        renderHotkeys();
        renderFavoritesSection();
        renderDataSection();
        renderVideoSettings();
        renderQuickSearches();
        renderQuickCards();
        renderFolders();
        setupRule34Account();
        setupGelbooruAccount();
        renderAbout();
        renderDeveloper();
        setupSettingsWindow();
        setupThumbnailCount();
        setupLoadingTips();
        setupVideoSettings();
        setupFavoriteFrame();
        setupSearchHistoryMenu();
        setupPoolSaving();
        setupPoolsBrowser();
        setupSearchSortMenu();
        setupTagAnalysis();
        loadLocalCopies();
        // Offline: filter the favorites once the list has loaded (and the copies are known).
        if (offlineMode && isFavoritesPage)
            controller()._model.personalListLoadedEvent.attach(() => { if (copiesLoaded) showOfflineFavorites(); });
        window.appInfo.onLocalCopiesChanged(loadLocalCopies);
        setupDownloadButton();
        setupJoiButton();
        setupLocalFileSizes();
        setupTouchMode();
        watchSlideChanges();
        updateForCurrentSlide();

        document.getElementById('tags-toggle-button').addEventListener('click', function () {
            setShowTags(!settings.showTags);
        });

        document.getElementById('scroll-top-button').addEventListener('click', scrollToImage);

        // A new search, filter or shuffle starts from the image again.
        for (let id of ['search-button', 'filter-button', 'randomize-button']) {
            let button = document.getElementById(id);
            if (button)
                button.addEventListener('click', scrollToImage);
        }

        let textBox = document.getElementById(isFavoritesPage ? 'filter-text' : 'search-text');
        textBox.addEventListener('keydown', function (e) {
            if (e.key == 'Enter')
                scrollToImage();
        });
    });
})();

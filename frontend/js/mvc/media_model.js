// What the models of the slideshow page and of the favorites page have in common: the settings of the
// slideshow (js/settings_table.js), the video volume, loading and saving them, and the favorites list.
// The page's own model tells how many slides there are, which one is shown and which come next
// (getSlideCount, getCurrentSlide, getCurrentSlideNumber, getNextSlidesForThumbnails).
class MediaModel
{
    constructor(settings)
    {
        this.view = null;

        this.videoVolume = 0;
        this.videoMuted = false;

        this.secondsPerSlide = 6;
        this.maxWidth = null;
        this.maxHeight = null;
        this.autoFitSlide = true;
        this.playVideosToEnd = false;

        this.isPlaying = false;
        this.timer = null;

        this.settings = settings;
        this.settingsByKey = Object.fromEntries(settings.map(setting => [setting.key, setting]));

        this.personalList = new PersonalList();

        this.currentSlideChangedEvent = new Event(this);
        this.playingChangedEvent = new Event(this);
        this.videoVolumeUpdatedEvent = new Event(this);
        this.settingUpdatedEvent = new Event(this);

        this.dataLoader = new DataLoader(this);
    }

    async loadUserSettings()
    {
        await this.dataLoader.loadUserSettings();
    }

    // The keys this page keeps in the storage.
    storageKeys()
    {
        return this.settings.map(setting => setting.key).concat(['videoVolume', 'videoMuted', 'personalListItems']);
    }

    // Puts what was stored in the model (and through its events in the controls).
    applyStoredSettings(stored)
    {
        if (stored.videoVolume != null && this.videoVolume != stored.videoVolume)
            this.setVideoVolume(stored.videoVolume);

        if (stored.videoMuted != null && this.videoMuted != stored.videoMuted)
            this.setVideoMuted(stored.videoMuted);

        for (let setting of this.settings)
        {
            let value = stored[setting.key];

            if (value == null)
                continue;

            if (setting.parse)
                value = setting.parse(value);

            if (value !== undefined)
                this.setSetting(setting.key, value);
        }

        // Both pages need the list.
        this.setPersonalList(stored.personalListItems == null ? this.personalList : new PersonalList(stored.personalListItems, this.dataLoader));
    }

    setSetting(key, value)
    {
        this[key] = value;

        this.dataLoader.save(key);

        this.settingUpdatedEvent.notify(key);
    }

    setVideoVolume(volume)
    {
        this.videoVolume = volume;

        this.dataLoader.save('videoVolume');

        this.videoVolumeUpdatedEvent.notify();
    }

    setVideoMuted(muted)
    {
        this.videoMuted = muted;

        this.dataLoader.save('videoMuted');

        this.videoVolumeUpdatedEvent.notify();
    }

    setPersonalList(personalList)
    {
        this.personalList = personalList;

        this.dataLoader.savePersonalList();
    }

    areMaxWithAndHeightEnabled()
    {
        return !this.autoFitSlide;
    }

    // Only the slideshow page has more slides to load.
    areThereMoreLoadableSlides()
    {
        return false;
    }

    waitForVideoToEndIfNeeded(callback)
    {
        if (this.playVideosToEnd && this.view != null)
            this.view.runWhenCurrentVideoEnds(callback);
        else
            callback();
    }
}

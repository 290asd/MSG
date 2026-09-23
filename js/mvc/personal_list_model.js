class PersonalListModel{
    constructor()
    {
        this.view = null;
        
        this.videoVolume = 0;
        this.videoMuted = false;
        
        this.filterText = "";
        // Site ID to show only that site's favorites; "" shows every site.
        this.siteFilter = "";

        this.secondsPerSlide = 6;
        this.maxWidth = null;
        this.maxHeight = null;
        this.autoFitSlide = true;
        this.playVideosToEnd = false;

        this.isPlaying = false;
        this.timer = null;
        this.timerMs = 0;

        this.sitesManager = null;

        this.personalList = new PersonalList();
        this.filtered = false;
        this.filteredPersonalList = null;

        this.currentSlideChangedEvent = new Event(this);
        this.playingChangedEvent = new Event(this);
        this.videoVolumeUpdatedEvent = new Event(this);
        this.secondsPerSlideUpdatedEvent = new Event(this);
        this.maxWidthUpdatedEvent = new Event(this);
        this.maxHeightUpdatedEvent = new Event(this);
        this.autoFitSlideUpdatedEvent = new Event(this);
        this.playVideosToEndUpdatedEvent = new Event(this);
        this.personalListLoadedEvent = new Event(this);

        this.dataLoader = new DataLoader(this);

        this.currentListItem = 0;

        // As many as fit on one row; set by app_settings.js from the window width.
        this.maxNumberOfThumbnails = 10;

        // Attached before the view's listeners, so preloading starts before the thumbnails are drawn.
        this.currentSlideChangedEvent.attach(() => this.preloadCurrentAndNextSlides());
        this.personalListLoadedEvent.attach(() => this.preloadCurrentAndNextSlides());
    }

    loadUserSettings()
    {
        this.dataLoader.loadUserSettings();
    }

    performFilter(filterText)
    {
        var filterWordsAsArray = filterText.split(" ");

        var orTags = filterWordsAsArray.filter(tag => tag.startsWith("~"));
        var orRegex = new RegExp("\\s" + orTags.join("\\s|\\s"));
        orRegex = new RegExp(orRegex.toString().replace(/~/g, "").slice(1, -1) + "\\s", "gi");
        
        var notTags = filterWordsAsArray.filter(tag => tag.startsWith("-"));
        var notRegex = new RegExp("\\s" + notTags.join("\\s|\\s"));
        notRegex = new RegExp(notRegex.toString().replace(/-/g, "").slice(1, -1) + "\\s", "gi");
        
        this.filtered = true;

        var siteItems = this.personalList.personalListItems.filter(item =>
            (this.siteFilter == "" || item.siteId == this.siteFilter) &&
            (!offlineMode || displayUrl(item.fileUrl) != item.fileUrl)); // offline: only the downloaded ones

        var items = filterText == "" ? siteItems : siteItems.filter((item) => {
            var passedOr = true;
            var passedWild = true;

            if (!item.tags ||
                typeof item.tags != "string" ||
                item.tags == "")
                return false;
            
            let tags = " " + item.tags.split(" ").join("  ") + " ";
            
            for(let i = 0; i < filterWordsAsArray.length; i++){
                let filterWord = filterWordsAsArray[i];

                if (filterWord.startsWith("-") && !filterWord.endsWith("*"))
                {
                    let matched = tags.match(notRegex);
                    if (matched)
                        return false;
                }
                else if (filterWord.startsWith("-") && filterWord.endsWith("*") && item.tags.includes(" " + filterWord.slice(1, -1)))
                {
                    return false;
                }
                else if (filterWord.startsWith("~"))
                {
                    let matched = tags.match(orRegex);
                    
                    passedOr = matched && matched.length > 0;
                }
                else if (filterWord.endsWith("*") && !filterWord.startsWith("-"))
                {
                    passedWild = item.tags.includes(filterWord.slice(0, -1));
                }
            }

            let noOrNotWildTags = filterWordsAsArray.filter(tag => !tag.startsWith("-") && !tag.startsWith("~") && !tag.endsWith("*"));
            let noOrNotWildRegex = new RegExp("\\s" + noOrNotWildTags.join("\\s|\\s"));
            noOrNotWildRegex = new RegExp(noOrNotWildRegex.toString().slice(1, -1) + "\\s", "gi");
            
            let matched = tags.match(noOrNotWildRegex);
            
            return (noOrNotWildTags.length == 0 || (matched != null && matched.length == noOrNotWildTags.length)) &&
                passedOr &&
                passedWild;
        });

        this.filteredPersonalList = new PersonalList(items)
        this.currentListItem = 1
        this.currentSlideChangedEvent.notify()
    }

    // Shows the current (possibly filtered) favorites in a new random order without changing the saved order.
    randomizeOrder()
    {
        let items = (this.filtered ? this.filteredPersonalList : this.personalList).personalListItems.slice();

        for (let i = items.length - 1; i > 0; i--)
        {
            let j = Math.floor(Math.random() * (i + 1));
            [items[i], items[j]] = [items[j], items[i]];
        }

        // Created empty so the copy doesn't start its own tagging of untagged items.
        this.filtered = true;
        this.filteredPersonalList = new PersonalList();
        this.filteredPersonalList.personalListItems = items;
        this.currentListItem = items.length > 0 ? 1 : 0;
        this.currentSlideChangedEvent.notify();
        this.restartSlideshowIfOn();
    }

    saveImportedFavorites()
    {
        this.dataLoader.savePersonalList();

        // Show the list in its saved order again so the imported favorites are included.
        this.filtered = false;
        this.filteredPersonalList = null;

        if (this.hasPersonalListItems() && this.currentListItem == 0)
            this.currentListItem = 1;

        this.personalListLoadedEvent.notify();
    }

    setSlideNumberToFirst()
    {
        if (this.currentListItem != 1)
        {
            this.currentListItem = 1;
            this.currentSlideChangedEvent.notify();
            this.restartSlideshowIfOn();
        }
    }

    decreaseCurrentSlideNumber()
    {
        if (this.currentListItem > 1)
        {
            this.currentListItem--;
            this.currentSlideChangedEvent.notify();
            this.restartSlideshowIfOn();
        }
    }

    increaseCurrentSlideNumber()
    {
        let listItemCount = this.filtered ? this.filteredPersonalList.count() : this.personalList.count();

        if (this.currentListItem < listItemCount)
        {
            this.currentListItem++;
            this.currentSlideChangedEvent.notify();
            this.restartSlideshowIfOn();
        }
    }
	
    decreaseCurrentSlideNumberByTen()
    {
        if (this.currentListItem > 1)
        {
            this.currentListItem -= 10;

            if (this.currentListItem < 1)
                this.currentListItem = 1;

            this.currentSlideChangedEvent.notify();
            this.restartSlideshowIfOn();
        }
    }
	
    increaseCurrentSlideNumberByTen()
    {
        let listItemCount = this.filtered ? this.filteredPersonalList.count() : this.personalList.count();
        
        if (this.currentListItem < listItemCount)
        {
            this.currentListItem += 10;

            if (this.currentListItem > listItemCount)
                this.currentListItem = listItemCount;

            this.currentSlideChangedEvent.notify();
            this.restartSlideshowIfOn();
        }
    }

    setSlideNumberToLast()
    {
        let listItemCount = this.filtered ? this.filteredPersonalList.count() : this.personalList.count();

        if (this.currentListItem != listItemCount)
        {
            this.currentListItem = listItemCount;
            this.currentSlideChangedEvent.notify();
            this.restartSlideshowIfOn();
        }
    }

    moveToThumbnailSlide(id)
    {
        var index = this.filtered ? this.filteredPersonalList.getIndexById(id) : this.personalList.getIndexById(id);

        if (index > -1)
        {
            this.currentListItem = index + 1;
            this.currentSlideChangedEvent.notify();
            this.restartSlideshowIfOn();
        }
    }

    // Loads the current slide and starts preloading the upcoming ones one at a time
    // (each finished thumbnail triggers the next, see PersonalListView.showThumbnails).
    preloadCurrentAndNextSlides()
    {
        let currentSlide = this.getCurrentSlide();

        if (currentSlide == null)
            return;

        currentSlide.preload();
        this.preloadNextUnpreloadedSlideAfterThisOneIfInRange(currentSlide);
    }

    preloadNextUnpreloadedSlideAfterThisOneIfInRange(slide)
    {
        let nextSlides = this.getNextListItemsForThumbnails();
        let nextUnpreloadedSlide = nextSlides.slice(nextSlides.indexOf(slide) + 1).find(s => !s.isPreloaded);

        if (nextUnpreloadedSlide)
            nextUnpreloadedSlide.preload();
    }

    tryToPlayOrPause()
    {
        if (this.hasPersonalListItems())
        {
            if (this.isPlaying)
                this.pauseSlideshow();
            else
                this.startSlideshow();
        }
    }

    startSlideshow()
    {
        this.tryToStartCountdown();

        this.isPlaying = true;

        this.playingChangedEvent.notify();
    }

    // Counts down only once the current slide has loaded, like the slideshow page.
    tryToStartCountdown()
    {
        let currentSlide = this.getCurrentSlide();

        if (currentSlide == null)
            return;

        if (currentSlide.isPreloaded)
        {
            this.startCountdown();
            return;
        }

        var _this = this;

        currentSlide.addCallback(function(){
            if (_this.isPlaying && currentSlide == _this.getCurrentSlide())
                _this.startCountdown();
        });

        currentSlide.preload();
    }

    startCountdown()
    {
        var millisecondsPerSlide = this.secondsPerSlide * 1000;

        var _this = this;

        clearTimeout(this.timer);

        this.timer = setTimeout(function() { _this.waitForVideoToEndIfNeeded(function() {
            if (_this.hasNextSlide())
            {
                // Continue slideshow
                _this.increaseCurrentSlideNumber();
            }
            else
            {
                // Loop when out of images/videos
                _this.setSlideNumberToFirst();
            }

        }); }, millisecondsPerSlide);
    }

    waitForVideoToEndIfNeeded(callback)
    {
        if (this.playVideosToEnd && this.view != null)
            this.view.runWhenCurrentVideoEnds(callback);
        else
            callback();
    }

    clearCallbackForCurrentSlide()
    {
        let currentSlide = this.getCurrentSlide();

        if (currentSlide != null)
            currentSlide.clearCallback();
    }

    restartSlideshowIfOn()
    {

        if (this.isPlaying)
        {
            clearTimeout(this.timer);
            if (this.view != null) this.view.cancelWaitingForVideoEnd();
            this.clearCallbackForCurrentSlide();

            this.tryToStartCountdown();
        }
    }

    pauseSlideshow()
    {
        clearTimeout(this.timer);
        if (this.view != null) this.view.cancelWaitingForVideoEnd();
        this.clearCallbackForCurrentSlide();

        this.isPlaying = false;

        this.playingChangedEvent.notify();
    }

    getPersonalListItemCount()
    {
        return this.filtered ? this.filteredPersonalList.count() : this.personalList.count();
    }

    hasPersonalListItems()
    {
        return (this.filtered ? this.filteredPersonalList.count() : this.personalList.count()) > 0;
    }

    hasNextSlide()
    {
        return (this.filtered ? this.filteredPersonalList.count() : this.personalList.count()) > this.getCurrentSlideNumber();
    }

    getCurrentSlide()
    {
        if (this.currentListItem == 0)
            return null;
        
        return this.filtered ? this.filteredPersonalList.get(this.currentListItem - 1) : this.personalList.get(this.currentListItem - 1);
    }

    getCurrentSlideNumber()
    {
        return this.currentListItem;
    }

    getNextListItemsForThumbnails()
    {
        let list = this.filtered ? this.filteredPersonalList : this.personalList;

        return list.getNextItemsForThumbnails(this.currentListItem - 1, this.maxNumberOfThumbnails);
    }

    areMaxWithAndHeightEnabled()
    {
        return !this.autoFitSlide;
    }

    removeCurrentImageFromFaves()
    {
        let currentSlide = this.getCurrentSlide();

        if (currentSlide == null)
            return;

        this.personalList.tryToRemove(currentSlide);

        RemoteFavorites.sync(currentSlide, false);

        // For the heart effect (app_settings.js).
        window.dispatchEvent(new CustomEvent('favorite-toggled', { detail: { faved: false } }));

        if (this.filtered)
            this.filteredPersonalList.tryToRemove(currentSlide);

        this.dataLoader.savePersonalList();

        if (this.currentListItem > (this.filtered ? this.filteredPersonalList.count() : this.personalList.count()))
            this.currentListItem = this.filtered ? this.filteredPersonalList.count() : this.personalList.count();

        this.personalListLoadedEvent.notify();
    }
	
    setVideoVolume(volume)
    {
        this.videoVolume = volume;

        this.dataLoader.saveVideoVolume();

        this.videoVolumeUpdatedEvent.notify();
    }
	
    setVideoMuted(muted)
    {
        this.videoMuted = muted;

        this.dataLoader.saveVideoMuted();

        this.videoVolumeUpdatedEvent.notify();
    }

    setSiteToSearch(site, checked)
    {
        this.sitesToSearch[site] = checked;

        this.dataLoader.saveSitesToSearch();

        this.sitesToSearchUpdatedEvent.notify();
    }
	
    setSecondsPerSlide(secondsPerSlide)
    {
        this.secondsPerSlide = secondsPerSlide;

        this.dataLoader.saveSecondsPerSlide();

        this.secondsPerSlideUpdatedEvent.notify();
    }
	
    setSecondsPerSlideIfValid(secondsPerSlide)
    {
		if (secondsPerSlide == '')
            return;

        if (isNaN(secondsPerSlide))
            return;

        if (secondsPerSlide < 1)
            return;

        this.setSecondsPerSlide(secondsPerSlide);
	}

    setMaxWidth(maxWidth)
    {
        this.maxWidth = maxWidth;

        this.dataLoader.saveMaxWidth();

        this.maxWidthUpdatedEvent.notify();
    }

    setMaxHeight(maxHeight)
    {
        this.maxHeight = maxHeight;

        this.dataLoader.saveMaxHeight();

        this.maxHeightUpdatedEvent.notify();
    }

    setAutoFitSlide(onOrOff)
    {
        this.autoFitSlide = onOrOff;

        this.dataLoader.saveAutoFitSlide();

        this.autoFitSlideUpdatedEvent.notify();
    }

    setPlayVideosToEnd(onOrOff)
    {
        this.playVideosToEnd = onOrOff;

        this.dataLoader.savePlayVideosToEnd();

        this.playVideosToEndUpdatedEvent.notify();
    }
	
    setIncludeImages(onOrOff)
    {
        this.includeImages = onOrOff;

        this.dataLoader.saveIncludeImages();

        this.includeImagesUpdatedEvent.notify();
    }
	
    setIncludeGifs(onOrOff)
    {
        this.includeGifs = onOrOff;

        this.dataLoader.saveIncludeGifs();

        this.includeGifsUpdatedEvent.notify();
    }
	
    setIncludeWebms(onOrOff)
    {
        this.includeWebms = onOrOff;

        this.dataLoader.saveIncludeWebms();

        this.includeWebmsUpdatedEvent.notify();
    }
	
    setHideBlacklist(onOrOff)
    {
        this.hideBlacklist = onOrOff;

        this.dataLoader.saveHideBlacklist();

        this.hideBlacklistUpdatedEvent.notify();
    }

    setBlacklist(blacklist)
    {
        this.blacklist = blacklist;

        this.dataLoader.saveBlacklist();

        this.blacklistUpdatedEvent.notify();
    }
	
    setDerpibooruApiKey(derpibooruApiKey)
    {
        this.derpibooruApiKey = derpibooruApiKey;

        this.dataLoader.saveDerpibooruApiKey();

        this.derpibooruApiKeyUpdatedEvent.notify();
    }

    setPersonalList(personalList)
    {
        this.personalList = personalList;

        this.dataLoader.savePersonalList();

        if (this.hasPersonalListItems() && this.currentListItem == 0)
            this.currentListItem = 1;

        this.personalListLoadedEvent.notify();
    }
}
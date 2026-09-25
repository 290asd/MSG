class SlideshowModel extends MediaModel
{
    constructor()
    {
        super(SLIDE_SETTINGS.concat(SEARCH_SETTINGS));

        this.searchText = "";

        this.sitesToSearch = {
            [SITE_DANBOORU]: false,
            [SITE_DERPIBOORU]: false,
            [SITE_E621]: false,
            [SITE_GELBOORU]: false,
            [SITE_KONACHAN]: false,
            [SITE_RULE34]: false,
            [SITE_SAFEBOORU]: true,
            [SITE_XBOORU]: false,
            [SITE_YANDERE]: false,
            [SITE_LOCAL]: false
        };

        this.includeImages = true;
        this.includeGifs = true;
        this.includeWebms = true;
        this.includeExplicit = false;
        this.includeQuestionable = false;
        this.includeSafe = true;
        this.includeDupes = false;
        this.hideBlacklist = false;
        this.blacklist = '';
        this.derpibooruApiKey = '';
        this.e621Login = '';
        this.e621ApiKey = '';
        this.gelbUserId = '';
        this.gelbApiKey = '';
        // Saved by the Rule34 fields in Settings → Sites & accounts (app_settings.js).
        this.rule34UserId = '';
        this.rule34ApiKey = '';
        this.storeHistory = true;
        this.searchHistory = [];

        this.sitesManager = null;

        this.sitesToSearchUpdatedEvent = new Event(this);
        this.searchHistoryUpdatedEvent = new Event(this);
        this.favoriteButtonUpdatedEvent = new Event(this);

        this.initialize();
    }

    initialize()
    {
        var numberOfSlidesToAlwaysHaveReadyToDisplay = 20;
        var maxNumberOfThumbnails = 10;

        this.sitesManager = new SitesManager(this, numberOfSlidesToAlwaysHaveReadyToDisplay, maxNumberOfThumbnails);

		var standardPageLimit = 100;

        this.sitesManager.addSite(SITE_DANBOORU, standardPageLimit);
        this.sitesManager.addSite(SITE_DERPIBOORU, 50);
        this.sitesManager.addSite(SITE_E621, standardPageLimit);
        this.sitesManager.addSite(SITE_GELBOORU, standardPageLimit);
        this.sitesManager.addSite(SITE_KONACHAN, standardPageLimit);
        this.sitesManager.addSite(SITE_RULE34, standardPageLimit);
        this.sitesManager.addSite(SITE_SAFEBOORU, standardPageLimit);
        this.sitesManager.addSite(SITE_XBOORU, standardPageLimit);
        this.sitesManager.addSite(SITE_YANDERE, standardPageLimit);
        this.sitesManager.addSite(SITE_LOCAL, standardPageLimit);
    }

    storageKeys()
    {
        return super.storageKeys().concat(['sitesToSearch', 'searchHistory', 'rule34UserId', 'rule34ApiKey']);
    }

    applyStoredSettings(stored)
    {
        super.applyStoredSettings(stored);

        if (stored.sitesToSearch != null)
        {
            // Only the sites this version knows.
            let cleanSitesToSearch = {};

            for (let site of Object.keys(this.sitesToSearch))
            {
                if (stored.sitesToSearch.hasOwnProperty(site))
                {
                    cleanSitesToSearch[site] = stored.sitesToSearch[site];
                }
            }

            this.setSitesToSearch(cleanSitesToSearch);
        }

        this.rule34UserId = stored.rule34UserId || '';
        this.rule34ApiKey = stored.rule34ApiKey || '';

        if (stored.searchHistory != null)
        {
            this.setSearchHistory(stored.searchHistory);
        }
    }

    pingSites()
    {
        console.log("Checking status of sites...");

		var _this = this;
		this.sitesManager.pingSites(function(siteManager){
			if (!siteManager.isOnline)
				_this.view.showSiteOffline(siteManager.id);
		});
	}

    performSearch(searchText)
    {
        this.sitesManager.resetConnections();

        var selectedSites = offlineMode ? [SITE_LOCAL] : poolIdFromSearch(searchText) ? [SITE_E621] : this.getSelectedSitesToSearch();
        this.sitesManager.enableSites(selectedSites);

        var _this = this;

        this.sitesManager.performSearch(searchText, function () {
			_this.view.clearInfoMessage();
            _this.currentSlideChangedEvent.notify();
        });

        this.storeSearchHistory(searchText);
    }

    storeSearchHistory(searchText)
    {
        if (!this.storeHistory)
            return;

        if (searchText == null || searchText.length == 0)
            return;

        if (this.searchHistory.includes(searchText))
        {
            var index = this.searchHistory.indexOf(searchText)

            if (index == 0)
                return;

            this.searchHistory.splice(index, 1);
            this.searchHistory.unshift(searchText);
        }
        else
        {
            this.searchHistory.unshift(searchText);
            this.searchHistory = this.searchHistory.slice(0, 100);
        }

        this.dataLoader.save('searchHistory');

        this.searchHistoryUpdatedEvent.notify();
    }

    areSomeTagsAreBlacklisted(tags)
    {
        // Split once per blacklist, not once per post.
        if (this.blacklistSet == null || this.blacklistSetSource !== this.blacklist)
        {
            this.blacklistSetSource = this.blacklist;
            this.blacklistSet = new Set(this.blacklist.trim().replace(/(\r\n|\n|\r)/gm," ").split(" "));
        }

        return tags.trim().split(" ").some(postTag => this.blacklistSet.has(postTag));
	}

    setSlideNumberToFirst()
    {
        this.sitesManager.moveToFirstSlide();
        this.currentSlideChangedEvent.notify();

        this.restartSlideshowIfOn();
    }

    decreaseCurrentSlideNumber()
    {
        if (!this.sitesManager.canDecreaseCurrentSlideNumber())
        {
            return;
        }

        this.sitesManager.decreaseCurrentSlideNumber();
        this.currentSlideChangedEvent.notify();

        this.restartSlideshowIfOn();
    }

    increaseCurrentSlideNumber()
    {
        var _this = this;

        this.sitesManager.increaseCurrentSlideNumber(function () {
            _this.currentSlideChangedEvent.notify();
        });

        this.restartSlideshowIfOn();
    }

    decreaseCurrentSlideNumberByTen()
    {
        if (!this.sitesManager.canDecreaseCurrentSlideNumber())
        {
            return;
        }

        this.sitesManager.decreaseCurrentSlideNumberByTen();
        this.currentSlideChangedEvent.notify();

        this.restartSlideshowIfOn();
    }

    increaseCurrentSlideNumberByTen()
    {
        var _this = this;

        this.sitesManager.increaseCurrentSlideNumberByTen(function () {
            _this.currentSlideChangedEvent.notify();
        });

        this.restartSlideshowIfOn();
    }

    setSlideNumberToLast()
    {
        var _this = this;

        this.sitesManager.moveToLastSlide(function () {
            _this.currentSlideChangedEvent.notify();
        });

        this.restartSlideshowIfOn();
    }

    moveToThumbnailSlide(id)
    {
        if (this.sitesManager.moveToThumbnailSlide(id))
        {
            this.currentSlideChangedEvent.notify();
        }
    }

    preloadNextUnpreloadedSlideAfterThisOneIfInRange(slide)
    {
        this.sitesManager.preloadNextUnpreloadedSlideAfterThisOneIfInRange(slide);
    }

    tryToPlayOrPause()
    {
        if (this.hasSlidesToDisplay())
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

    tryToStartCountdown()
    {
        if (this.sitesManager.isCurrentSlideLoaded())
        {
            this.startCountdown();
        }
        else
        {
            var _this = this;

            this.sitesManager.runCodeWhenCurrentSlideFinishesLoading(function(){
                _this.startCountdown();
            });
        }
    }

    startCountdown()
    {
        var millisecondsPerSlide = this.secondsPerSlide * 1000;

        var _this = this;

        this.timer = setTimeout(function() { _this.waitForVideoToEndIfNeeded(function() {
            if (_this.hasNextSlide())
            {
                // Continue slideshow
                _this.increaseCurrentSlideNumber();
            }
            else if (_this.isTryingToLoadMoreSlides())
            {
                // Wait for loading images/videos to finish
                _this.sitesManager.runCodeWhenFinishGettingMoreSlides(function(){
                    _this.tryToStartCountdown();
                });
            }
            else
            {
                // Loop when out of images/videos
                _this.setSlideNumberToFirst();
            }

        }); }, millisecondsPerSlide);
    }

    restartSlideshowIfOn()
    {
        if (this.isPlaying)
        {
            clearTimeout(this.timer);
            if (this.view != null) this.view.cancelWaitingForVideoEnd();
            this.sitesManager.clearCallbacksForPreloadingSlides();

            this.tryToStartCountdown();
        }
    }

    pauseSlideshow()
    {
        clearTimeout(this.timer);
        if (this.view != null) this.view.cancelWaitingForVideoEnd();
        this.sitesManager.clearCallbacksForPreloadingSlides();
        this.sitesManager.clearCallbacksForLoadingSlides();

        this.isPlaying = false;

        this.playingChangedEvent.notify();
    }

    hasAtLeastOneOnlineSiteSelected()
    {
        this.sitesManager.resetConnections();

        var selectedSites = this.getSelectedSitesToSearch();
        this.sitesManager.enableSites(selectedSites);

        return this.sitesManager.hasAtLeastOneOnlineSiteSelected();
    }

    getSlideCount()
    {
        return this.sitesManager.getTotalSlideNumber();
    }

    hasSlidesToDisplay()
    {
        return (this.getSlideCount() > 0);
    }

    hasNextSlide()
    {
        return (this.getSlideCount() > this.getCurrentSlideNumber());
    }

    isTryingToLoadMoreSlides()
    {
        return this.sitesManager.isTryingToLoadMoreSlides;
    }

    getCurrentSlide()
    {
        return this.sitesManager.getCurrentSlide();
    }

    getCurrentSlideNumber()
    {
        return this.sitesManager.currentSlideNumber;
    }

    areThereMoreLoadableSlides()
    {
        return this.sitesManager.areThereMoreLoadableSlides();
    }

    getNextSlidesForThumbnails()
    {
        return this.sitesManager.getNextSlidesForThumbnails();
    }

    getSelectedSitesToSearch()
    {
        var selectedSitesToSearch = [];

        for (var siteToSearch in this.sitesToSearch)
        {
            if (this.sitesToSearch[siteToSearch])
            {
                selectedSitesToSearch.push(siteToSearch);
            }
        }

        return selectedSitesToSearch;
    }

    setSitesToSearch(sitesToSearch)
    {
        this.sitesToSearch = sitesToSearch;

        this.dataLoader.save('sitesToSearch');

        this.sitesToSearchUpdatedEvent.notify();
    }

    setSiteToSearch(site, checked)
    {
        this.sitesToSearch[site] = checked;

        this.dataLoader.save('sitesToSearch');

        this.sitesToSearchUpdatedEvent.notify();
    }

    setSearchHistory(searchHistory)
    {
        this.searchHistory = searchHistory;

        this.dataLoader.save('searchHistory');

        this.searchHistoryUpdatedEvent.notify();
    }

    toggleSlideFave()
    {
        let currentSlide = this.getCurrentSlide();

        if (currentSlide == null)
            return;

        let faved = !this.isCurrentSlideFaved();

        if (faved)
        {
            this.personalList.tryToAdd(currentSlide);
        }
        else
        {
            this.personalList.tryToRemove(currentSlide);
        }

        this.dataLoader.savePersonalList();
        this.favoriteButtonUpdatedEvent.notify();

        RemoteFavorites.sync(currentSlide, faved);

        // For the heart effect (app_settings.js).
        window.dispatchEvent(new CustomEvent('favorite-toggled', { detail: { faved: faved } }));
    }

    isCurrentSlideFaved()
    {
        let currentSlide = this.getCurrentSlide();

        if (currentSlide == null)
            return false;

        return this.personalList.contains(currentSlide);
    }

    toggleTags()
    {
        this.view.toggleTags();
    }
}

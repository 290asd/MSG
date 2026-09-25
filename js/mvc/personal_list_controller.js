class PersonalListController
{
    constructor(uiElements)
    {
        this._model = new PersonalListModel();
        this._view = new PersonalListView(this._model, uiElements);
        this._model.view = this._view;

        setTimeout(() => {
            if(!this._model.personalList.indexed){
                this._view.displayWarningMessage("Your favorites are not indexed, filtering will be inaccurate.")
                var checkInterval = setInterval(() => {
                    if(this._model.personalList.indexed){ 
                        this._view.clearWarningMessage()
                        clearInterval(checkInterval)
                    }
                }, 5000)
            }
        }, 1000)

        this._view.updateOptions();

        var _this = this;

        // Attach view listeners
        this._view.currentImageClickedEvent.attach(function() {
            _this.currentSlideClicked();
        });
        
        this._view.currentVideoClickedEvent.attach(function() {
            _this.currentSlideClicked();
        });
        
        this._view.currentVideoVolumeChangedEvent.attach(function() {
            _this.videoVolumeChanged();
        });

        this._view.firstNavButtonClickedEvent.attach(function () {
            _this.firstNavButtonClicked();
        });

        this._view.previousNavButtonClickedEvent.attach(function () {
            _this.previousNavButtonClicked();
        });

        this._view.nextNavButtonClickedEvent.attach(function () {
            _this.nextNavButtonClicked();
        });

        this._view.lastNavButtonClickedEvent.attach(function () {
            _this.lastNavButtonClicked();
        });
        
        this._view.goBackTenImagesPressedEvent.attach(function () {
            _this.goBackTenImagesPressed();
        });
        
        this._view.goForwardTenImagesPressedEvent.attach(function () {
            _this.goForwardTenImagesPressed();
        });

        this._view.playButtonClickedEvent.attach(function () {
            _this.playButtonClicked();
        });

        this._view.pauseButtonClickedEvent.attach(function () {
            _this.pauseButtonClicked();
        });

        this._view.enterKeyPressedOutsideOfFilterTextBoxEvent.attach(function () {
            _this.enterKeyPressedOutsideOfFilterTextBox();
        });

        this._view.filterTextChangedEvent.attach(function () {
            _this.filterTextChanged();
        });

        this._view.enterKeyPressedInFilterTextBoxEvent.attach(function () {
            _this.enterKeyPressedInFilterTextBox();
        });

        this._view.filterButtonClickedEvent.attach(function () {
            _this.filterButtonClicked();
        });

        this._view.siteFilterChangedEvent.attach(function () {
            _this.siteFilterChanged();
        });

        this._view.randomizeButtonClickedEvent.attach(function () {
            _this.randomizeButtonClicked();
        });

        this._view.secondsPerSlideChangedEvent.attach(function () {
            _this.secondsPerSlideChanged();
        });

        this._view.maxWidthChangedEvent.attach(function () {
            _this.maxWidthChanged();
        });

        this._view.maxHeightChangedEvent.attach(function () {
            _this.maxHeightChanged();
        });

        this._view.autoFitSlideChangedEvent.attach(function () {
            _this.autoFitSlideChanged();
        });

        this._view.playVideosToEndChangedEvent.attach(function () {
            _this.playVideosToEndChanged();
        });

        this._view.removeCurrentImageFromFavesPressedEvent.attach(function () {
            _this.removeCurrentImageFromFavesPressed();
        });

        this._model.loadUserSettings();
        
        this._view.updateSlidesAndNavigation();
    }

    currentSlideClicked()
    {
        var currentSlide = this._model.getCurrentSlide();

        if (currentSlide == null || !window.clickOpensPost())
            return;

        this._view.openUrlInNewWindow(currentSlide.viewableWebsitePostUrl);

        this._model.pauseSlideshow();
    }
	
    videoVolumeChanged()
    {
        var videoVolume = this._view.getVideoVolume();
        var videoMuted = this._view.getVideoMuted();
		
        this._model.setVideoVolume(videoVolume);
        this._model.setVideoMuted(videoMuted);
    }

    firstNavButtonClicked()
    {
        this._model.setSlideNumberToFirst();
    }

    previousNavButtonClicked()
    {
        this._model.decreaseCurrentSlideNumber();
    }

    nextNavButtonClicked()
    {
        this._model.increaseCurrentSlideNumber();
    }

    lastNavButtonClicked()
    {
        this._model.setSlideNumberToLast();
    }
	
    goBackTenImagesPressed()
    {
        this._model.decreaseCurrentSlideNumberByTen();
    }
	
    goForwardTenImagesPressed()
    {
        this._model.increaseCurrentSlideNumberByTen();
    }

    playButtonClicked()
    {
        this._model.startSlideshow();
    }

    pauseButtonClicked()
    {
        this._model.pauseSlideshow();
    }

    enterKeyPressedOutsideOfFilterTextBox()
    {
        this._model.tryToPlayOrPause();
    }

    filterTextChanged()
    {
        this._model.filterText = this._view.getFilterText().trim();
    }

    enterKeyPressedInFilterTextBox()
    {
        this._model.filterText = this._view.getFilterText().trim();
		this._view.removeFocusFromFilterTextBox();
        this.filterButtonClicked();
    }

    filterButtonClicked()
    {
        this._view.clearUI();
        this._view.removeFocusFromFilterButton();

        let filterText = this._model.filterText;

        if (filterText == "" && this._model.siteFilter == "" && !offlineMode)
        {
            this._model.filtered = false;
            this._model.filteredPersonalList = null;
            this._view.clearUI();
            this._view.removeFocusFromFilterButton();
            this._model.currentListItem = 1;
            this._model.currentSlideChangedEvent.notify();

            return;
        }
		
        this._model.performFilter(filterText);
    }

    siteFilterChanged()
    {
        this._model.siteFilter = this._view.getSiteFilter();
        this._model.filterText = this._view.getFilterText().trim();
        this.filterButtonClicked();
    }

    randomizeButtonClicked()
    {
        this._view.clearUI();
        this._model.randomizeOrder();
    }

    // Saves the favorites being shown (all, filtered or random) to Downloads/MSG/favorites.
    // Called again while downloading, it cancels. Progress goes to report (the settings window's status line).
    async downloadFavoritesButtonClicked(report)
    {
        if (this.isDownloadingFavorites)
        {
            chrome.downloads.cancelDownloadMany();
            return;
        }

        let list = this._model.filtered ? this._model.filteredPersonalList : this._model.personalList;
        let urls = list.personalListItems.map(item => item.fileUrl).filter(url => url);

        if (urls.length == 0)
        {
            report('There are no favorites to download.');
            return;
        }

        this.isDownloadingFavorites = true;

        let describe = (p) => (p.downloaded + p.skipped + p.failed) + ' / ' + p.total + ' (' +
            p.downloaded + ' downloaded, ' + p.skipped + ' already saved' + (p.failed ? ', ' + p.failed + ' failed' : '') + ')';

        report('Downloading favorites: 0 / ' + urls.length);

        let result = await chrome.downloads.downloadMany(urls, 'MSG/favorites', (progress) => {
            report('Downloading favorites: ' + describe(progress));
        });

        this.isDownloadingFavorites = false;
        report((result.cancelled ? 'Download cancelled: ' : 'Download finished: ') + describe(result) + '. Saved to ' + result.folder);
    }

    secondsPerSlideChanged()
    {
        var secondsPerSlideText = this._view.getSecondsPerSlide();

        this._model.setSecondsPerSlideIfValid(secondsPerSlideText);
    }

    maxWidthChanged()
    {
        var maxWidthText = this._view.getMaxWidth();

        if (maxWidthText == '')
        {
			maxWidthText = null;
        }
		else if (isNaN(maxWidthText))
			return;
		else if (maxWidthText < 1)
            return;
        
        this._model.setMaxWidth(maxWidthText);
    }

    maxHeightChanged()
    {
        var maxHeightText = this._view.getMaxHeight();
        
        if (maxHeightText == '') {
            maxHeightText = null;
        }
        else if (isNaN(maxHeightText))
            return;
        else if (maxHeightText < 1)
            return;
        
        this._model.setMaxHeight(maxHeightText);
    }

    playVideosToEndChanged()
    {
        this._model.setPlayVideosToEnd(this._view.getPlayVideosToEnd());
    }

    autoFitSlideChanged()
    {
        var autoFitSlide = this._view.getAutoFitSlide();

        this._model.setAutoFitSlide(autoFitSlide);
    }

    removeCurrentImageFromFavesPressed()
    {
        this._model.removeCurrentImageFromFaves();
    }
}
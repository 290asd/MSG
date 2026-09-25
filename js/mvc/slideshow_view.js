class SlideshowView extends MediaView
{
    constructor (slideshowModel, uiElements) {
        super(slideshowModel, uiElements);

        this.searchButtonClickedEvent = new Event(this);
        this.enterKeyPressedOutsideOfSearchTextBoxEvent = new Event(this);
        this.searchTextChangedEvent = new Event(this);
        this.enterKeyPressedInSearchTextBoxEvent = new Event(this);
        this.sitesToSearchChangedEvent = new Event(this);
        this.clearHistoryClickedEvent = new Event(this);
        this.favoriteKeyPressedEvent = new Event(this);
        this.favoriteButtonClickedEvent = new Event(this);

        this.attachModelListeners();

        this.attachUiElementListeners();

        this.setupLoadingAnimation();

        this.setFocusToSearchBox();
    }

    attachModelListeners()
    {
        super.attachModelListeners();

        var _this = this;

        this._model.sitesToSearchUpdatedEvent.attach(function () {
            _this.updateSitesToSearch();
        });

        this._model.searchHistoryUpdatedEvent.attach(function () {
            _this.updateSearchHistory();
        });

        this._model.favoriteButtonUpdatedEvent.attach(function (){
            _this.updateFavoriteButton();
        });
    }

    attachUiElementListeners()
    {
        super.attachUiElementListeners();

        var _this = this;

        document.addEventListener('keydown', function (e) {
            // Hotkeys are off while the settings window is open.
            if (document.querySelector('dialog[open]'))
                return;

            var key = e.which || e.keyCode;

            if (!(
                key == ENTER_KEY_ID ||
                key == SPACE_KEY_ID ||
                key == LEFT_ARROW_KEY_ID ||
                key == RIGHT_ARROW_KEY_ID ||
                key == A_KEY_ID ||
                key == W_KEY_ID ||
                key == S_KEY_ID ||
                key == D_KEY_ID ||
                key == F_KEY_ID ||
                key == L_KEY_ID ||
                key == G_KEY_ID ||
                key == E_KEY_ID ||
                key == R_KEY_ID))
            {
                return;
            }

            if (document.activeElement !== _this.uiElements.searchTextBox &&
                document.activeElement !== _this.uiElements.secondsPerSlideTextBox &&
                document.activeElement !== _this.uiElements.maxWidthTextBox &&
                document.activeElement !== _this.uiElements.maxHeightTextBox &&
                document.activeElement !== _this.uiElements.blacklist &&
                document.activeElement !== _this.uiElements.derpibooruApiKey &&
                document.activeElement !== _this.uiElements.e621Login &&
                document.activeElement !== _this.uiElements.e621ApiKey &&
                document.activeElement !== _this.uiElements.gelbUserId &&
                document.activeElement !== _this.uiElements.gelbApiKey
                ) {

                if (key == LEFT_ARROW_KEY_ID || key == A_KEY_ID)
                    _this.previousNavButtonClickedEvent.notify();
                if (key == RIGHT_ARROW_KEY_ID || key == D_KEY_ID)
                    _this.nextNavButtonClickedEvent.notify();
                if (key == W_KEY_ID)
                    _this.goBackTenImagesPressedEvent.notify();
                if (key == S_KEY_ID)
                    _this.goForwardTenImagesPressedEvent.notify();
                if (key == ENTER_KEY_ID || key == SPACE_KEY_ID)
                {
                    if (document.activeElement !== _this.uiElements.searchButton)
                        _this.enterKeyPressedOutsideOfSearchTextBoxEvent.notify();
                }
                if (key == SPACE_KEY_ID)
                {
                    e.preventDefault();
                }
                if (key == F_KEY_ID)
                {
                    _this._model.setSetting('autoFitSlide', !_this._model.autoFitSlide);
                }
                if (key == L_KEY_ID)
                {
                    _this.downloadCurrentSlide();
                }
                if (key == G_KEY_ID)
                {
                    _this.favoriteKeyPressedEvent.notify();
                }
                if (key == E_KEY_ID)
                {
                    _this.openCurrentSlideSource();
                }
                if (key == R_KEY_ID)
                {
                    _this._model.toggleTags();
                }
            }
        });

        this.uiElements.searchTextBox.addEventListener('change', function () {
            _this.searchTextChangedEvent.notify();
        });

        this.uiElements.searchTextBox.addEventListener('keypress', function (e) {
            var key = e.which || e.keyCode;

            if (key == ENTER_KEY_ID) {
                _this.enterKeyPressedInSearchTextBoxEvent.notify();
            }
        });

        this.uiElements.searchButton.addEventListener('click', function () {
            _this.searchButtonClickedEvent.notify();
        });

        for (let siteToSearch of this.uiElements.sitesToSearch)
        {
            siteToSearch.addEventListener('change', function (e) {
                _this.sitesToSearchChangedEvent.notify({
                    checked: e.target.checked,
                    site: e.target.value
                });
            });
        }

        this.uiElements.clearHistoryButton.addEventListener('click', function () {
            _this.clearHistoryClickedEvent.notify();
        });

        this.uiElements.favoriteButton.addEventListener('click', function() {
            _this.favoriteButtonClickedEvent.notify();
        });
    }

    updateSlidesAndNavigation() {
        super.updateSlidesAndNavigation();
        this.toggleTags(true);
    }

    updateSlides() {
        this.displayCurrentSlide();
        this.updateFavoriteButton();
        this.showThumbnails();
    }

    displayCurrentSlide() {
        if (this._model.hasSlidesToDisplay())
        {
            this.displaySlide();
        }
        else if (this.isDisplayingWarningMessage())
        {
            // Current warning message more important
        }
        else
        {
            // Says what was searched, where, and for which files, e.g. No images or videos were found for "sky" on rule34.xxx.
            var includingImagesOrGifs = (this._model.includeImages || this._model.includeGifs);
            var what = includingImagesOrGifs && this._model.includeWebms ? 'images or videos' : includingImagesOrGifs ? 'images' : 'videos';
            var sites = this._model.sitesManager.siteManagers.filter(m => m.isEnabled)
                .map(m => m.id == SITE_LOCAL ? 'your folders' : m.url.replace(/^https?:\/\//, ''));
            var searchText = (this._model.searchText || '').trim();

            var message = 'No ' + what + ' were found' + (searchText ? ' for "' + searchText + '"' : '') + (sites.length ? ' on ' + sites.join(', ') : '') + '.';
            if (!(this._model.includeExplicit && this._model.includeQuestionable && this._model.includeSafe))
                message += ' Only the ratings chosen in Settings → Filtering are shown.';

            this.displayWarningMessage(message);
        }
    }

    updateNavigation() {
        if (this._model.hasSlidesToDisplay())
        {
            this.updateNavigationButtonsAndDisplay();
            this.showNavigation();
        }
        else
        {
            this.hideNavigation();
        }
    }

    getSearchText() {
        return this.uiElements.searchTextBox.value;
    }

    setFocusToSearchBox() {
        this.uiElements.searchTextBox.focus();
    }

    removeFocusFromSearchTextBox() {
        this.uiElements.searchTextBox.blur();
    }

    removeFocusFromSearchButton() {
        this.uiElements.searchButton.blur();
    }

    updateSitesToSearch() {
        for (let siteToSearch of this.uiElements.sitesToSearch) {
            var site = siteToSearch.value;
            var checked = this._model.sitesToSearch[site];

            siteToSearch.checked = checked;

            if (site == SITE_DERPIBOORU)
            {
                this.uiElements.derpibooruApiKeyContainer.style.display = checked ? 'inline' : 'none';
            }

            if (site == SITE_E621)
            {
                this.uiElements.e621LoginContainer.style.display = checked ? 'inline' : 'none';
                this.uiElements.e621ApiKeyContainer.style.display = checked ? 'inline' : 'none';
            }

            if (site == SITE_GELBOORU)
            {
                this.uiElements.gelbUserIdContainer.style.display = checked ? 'inline' : 'none';
                this.uiElements.gelbApiKeyContainer.style.display = checked ? 'inline' : 'none';
            }
        }
    }

    updateSearchHistory() {
        this.uiElements.searchHistory.replaceChildren(...this._model.searchHistory.map(function (searchHistoryItem) {
            var optionElement = document.createElement("option");
            optionElement.value = searchHistoryItem;
            return optionElement;
        }));
    }

    showSiteOffline(site) {
        for (let siteToSearch of this.uiElements.sitesToSearch)
        {
            if (siteToSearch.value == site)
            {
                siteToSearch.parentElement.classList.add("siteOffline");
                return;
            }
        }
    }

    hideOrShowBlacklist()
    {
        this.uiElements.blacklistContainer.style.display = this._model.hideBlacklist ? "none" : "block";
    }

    updateFavoriteButton() {
        this.uiElements.favoriteButton.classList.toggle("faved", this._model.isCurrentSlideFaved());
    }

    toggleTags(update)
    {
        if(this.uiElements.tags.style.display == "none" && !update)
        {
            this.uiElements.tags.style.display = "block";
            this.uiElements.tags.textContent = this._model.getCurrentSlide().tags;
        }
        else if (this.uiElements.tags.style.display == "block" && update)
        {
            this.uiElements.tags.textContent = this._model.getCurrentSlide().tags;
        }
        else
        {
            this.uiElements.tags.style.display = "none";
            this.uiElements.tags.textContent = "";
        }
    }
}

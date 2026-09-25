class PersonalListView extends MediaView
{
    constructor (personalListModel, uiElements) {
        super(personalListModel, uiElements);

        this.currentImageClickedEvent = new Event(this);
        this.currentVideoClickedEvent = new Event(this);
        this.filterButtonClickedEvent = new Event(this);
        this.randomizeButtonClickedEvent = new Event(this);
        this.siteFilterChangedEvent = new Event(this);
        this.enterKeyPressedOutsideOfFilterTextBoxEvent = new Event(this);
        this.filterTextChangedEvent = new Event(this);
        this.enterKeyPressedInFilterTextBoxEvent = new Event(this);
        this.removeCurrentImageFromFavesPressedEvent = new Event(this);

        this.attachModelListeners();

        this.attachUiElementListeners();

        this.setupLoadingAnimation();
    }

    attachModelListeners()
    {
        super.attachModelListeners();

        var _this = this;

        this._model.personalListLoadedEvent.attach(function () {
            _this.clearWarningMessage();
            _this.clearInfoMessage();
            _this.updateSlidesAndNavigation();
            _this.updateSiteFilterOptions();
        });
    }

    // Lists only the sites that have favorites, with how many each has.
    updateSiteFilterOptions()
    {
        let select = this.uiElements.siteFilterSelect;
        let counts = {};
        for (let item of this._model.personalList.personalListItems)
            counts[item.siteId] = (counts[item.siteId] || 0) + 1;

        let names = { ATFB: 'ATFBooru', DANB: 'Danbooru', DERP: 'Derpibooru', E621: 'e621', GELB: 'Gelbooru', KONA: 'Konachan', RULE: 'Rule34', SAFE: 'Safebooru', XBOO: 'Xbooru', YAND: 'Yande.re', LOCL: 'Your folders' };
        let total = this._model.personalList.personalListItems.length;
        select.replaceChildren(new Option('All sites (' + total + ')', ''));
        for (let id of Object.keys(counts).sort((a, b) => counts[b] - counts[a]))
            select.add(new Option((names[id] || id) + ' (' + counts[id] + ')', id));
        select.value = counts[this._model.siteFilter] ? this._model.siteFilter : '';
    }

    getSiteFilter() {
        return this.uiElements.siteFilterSelect.value;
    }

    attachUiElementListeners()
    {
        super.attachUiElementListeners();

        var _this = this;

        this.uiElements.currentImage.addEventListener('click', function() {
            _this.currentImageClickedEvent.notify();
        });

        this.uiElements.currentVideo.addEventListener('click', function() {
            _this.currentVideoClickedEvent.notify();
        });

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
                key == E_KEY_ID))
            {
                return;
            }

            if (document.activeElement !== _this.uiElements.filterTextBox &&
                document.activeElement !== _this.uiElements.secondsPerSlideTextBox &&
                document.activeElement !== _this.uiElements.maxWidthTextBox &&
                document.activeElement !== _this.uiElements.maxHeightTextBox) {

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
                    if (document.activeElement !== _this.uiElements.filterButton)
                        _this.enterKeyPressedOutsideOfFilterTextBoxEvent.notify();
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
                    _this.removeCurrentImageFromFavesPressedEvent.notify();
                }
                if (key == E_KEY_ID)
                {
                    _this.openCurrentSlideSource();
                }
            }
        });

        this.uiElements.filterTextBox.addEventListener('change', function () {
            _this.filterTextChangedEvent.notify();
        });

        this.uiElements.filterTextBox.addEventListener('keypress', function (e) {
            var key = e.which || e.keyCode;

            if (key == ENTER_KEY_ID) {
                _this.enterKeyPressedInFilterTextBoxEvent.notify();
            }
        });

        this.uiElements.filterButton.addEventListener('click', function () {
            _this.filterButtonClickedEvent.notify();
        });

        this.uiElements.siteFilterSelect.addEventListener('change', function () {
            _this.uiElements.siteFilterSelect.blur();
            _this.siteFilterChangedEvent.notify();
        });

        this.uiElements.randomizeButton.addEventListener('click', function () {
            _this.uiElements.randomizeButton.blur();
            _this.randomizeButtonClickedEvent.notify();
        });
    }

    displayCurrentSlide() {
        if (this._model.hasPersonalListItems())
        {
            this.displaySlide();
        }
        else if (this.isDisplayingWarningMessage())
        {
            // Current warning message more important
        }
        else
        {
            var message = '';

            var includingImagesOrGifs = (this._model.includeImages || this._model.includeGifs);

            if (includingImagesOrGifs && this._model.includeWebms)
                message = 'No images or videos were found.';
            else if (includingImagesOrGifs && !this._model.includeWebms)
                message = 'No images were found.';
            else if (!includingImagesOrGifs && this._model.includeWebms)
                message = 'No videos were found.';

            this.displayWarningMessage(message);
        }
    }

    updateNavigation() {
        if (this._model.hasPersonalListItems())
        {
            this.updateNavigationButtonsAndDisplay();
            this.showNavigation();
        }
        else if (this._model.filtered)
        {
            this.hideNavigation();
            this.displayWarningMessage(this.noMatchMessage());
        }
        else
        {
            this.hideNavigation();
            this.displayInfoMessage("No images have been faved yet.");
        }
    }

    // A filter that found nothing says so, and points out tags that are written with _ (dark nek0gami → dark_nek0gami).
    noMatchMessage() {
        let text = this._model.filterText.trim();
        let message = text ? 'No favorites match "' + text + '"' : 'No favorites here';
        if (this._model.siteFilter)
            message += ' on the chosen site';
        if (typeof offlineMode != 'undefined' && offlineMode)
            message += ' among the downloaded ones';
        message += '.';

        let words = text.toLowerCase().split(/\s+/).filter(word => word && !/^[-~]/.test(word));
        let joined = words.join('_');
        if (words.length > 1 && this._model.personalList.personalListItems.some(item => typeof item.tags == 'string' && (' ' + item.tags.toLowerCase() + ' ').includes(' ' + joined + ' ')))
            message += ' Tags with several words are written with _: try ' + joined + '.';

        return message;
    }

    getFilterText() {
        return this.uiElements.filterTextBox.value;
    }

    removeFocusFromFilterTextBox() {
        this.uiElements.filterTextBox.blur();
    }

    removeFocusFromFilterButton() {
        this.uiElements.filterButton.blur();
    }
}

class SlideshowController extends MediaController
{
    constructor(uiElements)
    {
        var model = new SlideshowModel();
        var view = new SlideshowView(model, uiElements);

        super(model, view);

        this._view.updateSitesToSearch();
        this._view.updateOptions();

        var _this = this;

        this._view.enterKeyPressedOutsideOfSearchTextBoxEvent.attach(function () {
            _this._model.tryToPlayOrPause();
        });

        this._view.searchTextChangedEvent.attach(function () {
            _this.searchTextChanged();
        });

        this._view.enterKeyPressedInSearchTextBoxEvent.attach(function () {
            _this.enterKeyPressedInSearchTextBox();
        });

        this._view.searchButtonClickedEvent.attach(function () {
            _this.searchButtonClicked();
        });

        this._view.sitesToSearchChangedEvent.attach(function (args) {
            _this._model.setSiteToSearch(args.site, args.checked);
        });

        this._view.clearHistoryClickedEvent.attach(function () {
            _this._model.setSearchHistory([]);
        });

        this._view.favoriteKeyPressedEvent.attach(function () {
            _this._model.toggleSlideFave();
        });

        this._view.favoriteButtonClickedEvent.attach(function () {
            _this._model.toggleSlideFave();
        });
    }

    async initialize()
    {
        await this._model.loadUserSettings();
        this._model.pingSites();
    }

    searchTextChanged()
    {
        this._model.searchText = this._view.getSearchText();
    }

    enterKeyPressedInSearchTextBox()
    {
        this._model.searchText = this._view.getSearchText();
		this._view.removeFocusFromSearchTextBox();
        this.searchButtonClicked();
    }

    searchButtonClicked()
    {
        this._view.clearUI();
        this._view.removeFocusFromSearchButton();

        var searchText = this._model.searchText;

        // Your folders can be browsed without search words (offline, only they are searched).
        if (searchText == '' && !this._model.sitesToSearch[SITE_LOCAL] && !offlineMode)
        {
            this._view.displayWarningMessage('The search query is blank.');
            return;
        }

        let poolSearch = poolIdFromSearch(searchText) != null || offlineMode;

        if (poolSearch && !offlineMode && !this._model.sitesManager.siteManagers.some(m => m.id == SITE_E621 && m.isOnline))
        {
            this._view.displayWarningMessage('e621 is offline, so the pool can\'t be loaded.');
            return;
        }

        if (!poolSearch && !this._model.hasAtLeastOneOnlineSiteSelected())
        {
            this._view.displayWarningMessage('No online sites were selected to be searched.');
            return;
        }

		var includingImagesOrGifs = (this._model.includeImages || this._model.includeGifs);

		if (!includingImagesOrGifs && !this._model.includeWebms)
		{
			this._view.displayWarningMessage('You must select at least one of: Images, GIFs, and WEBMs.');
            return;
		}

		var message = '';

		if (includingImagesOrGifs && this._model.includeWebms)
			message = 'Searching for images and videos...';
		else if (includingImagesOrGifs && !this._model.includeWebms)
			message = 'Searching for images...';
		else if (!includingImagesOrGifs && this._model.includeWebms)
			message = 'Searching for videos...';

		this._view.displayInfoMessage(message);

        this._model.performSearch(searchText);
    }
}

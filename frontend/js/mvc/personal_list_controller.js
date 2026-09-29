class PersonalListController extends MediaController
{
    constructor(uiElements)
    {
        var model = new PersonalListModel();
        var view = new PersonalListView(model, uiElements);

        super(model, view);

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

        this._view.currentImageClickedEvent.attach(function() {
            _this.currentSlideClicked();
        });

        this._view.currentVideoClickedEvent.attach(function() {
            _this.currentSlideClicked();
        });

        this._view.enterKeyPressedOutsideOfFilterTextBoxEvent.attach(function () {
            _this._model.tryToPlayOrPause();
        });

        this._view.filterTextChangedEvent.attach(function () {
            _this._model.filterText = _this._view.getFilterText().trim();
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

        this._view.removeCurrentImageFromFavesPressedEvent.attach(function () {
            _this._model.removeCurrentImageFromFaves();
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
}

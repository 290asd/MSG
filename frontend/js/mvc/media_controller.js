// What the controllers of the slideshow page and of the favorites page have in common: the navigation
// buttons, the video volume and the settings of js/settings_table.js.
class MediaController
{
    constructor(model, view)
    {
        this._model = model;
        this._view = view;
        this._model.view = this._view;

        var _this = this;

        // Attach view listeners
        this._view.currentVideoVolumeChangedEvent.attach(function() {
            _this.videoVolumeChanged();
        });

        this._view.firstNavButtonClickedEvent.attach(function () {
            _this._model.setSlideNumberToFirst();
        });

        this._view.previousNavButtonClickedEvent.attach(function () {
            _this._model.decreaseCurrentSlideNumber();
        });

        this._view.nextNavButtonClickedEvent.attach(function () {
            _this._model.increaseCurrentSlideNumber();
        });

        this._view.lastNavButtonClickedEvent.attach(function () {
            _this._model.setSlideNumberToLast();
        });

        this._view.goBackTenImagesPressedEvent.attach(function () {
            _this._model.decreaseCurrentSlideNumberByTen();
        });

        this._view.goForwardTenImagesPressedEvent.attach(function () {
            _this._model.increaseCurrentSlideNumberByTen();
        });

        this._view.playButtonClickedEvent.attach(function () {
            _this._model.startSlideshow();
        });

        this._view.pauseButtonClickedEvent.attach(function () {
            _this._model.pauseSlideshow();
        });

        this._view.settingChangedEvent.attach(function (key) {
            _this.settingChanged(key);
        });
    }

    videoVolumeChanged()
    {
        this._model.setVideoVolume(this._view.getVideoVolume());
        this._model.setVideoMuted(this._view.getVideoMuted());
    }

    // What was typed in the control goes to the model, unless it isn't a value the setting can have.
    settingChanged(key)
    {
        var value = this._view.getSetting(key);
        var parse = this._model.settingsByKey[key].parse;

        if (parse)
        {
            value = parse(value);

            if (value === undefined)
                return;
        }

        this._model.setSetting(key, value);
    }
}

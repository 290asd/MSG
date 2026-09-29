// What the views of the slideshow page and of the favorites page have in common: the image or video on the
// screen and its size, the navigation buttons, the thumbnails and the controls of the settings in
// js/settings_table.js. A page's own view shows its messages when there is nothing to display
// (displayCurrentSlide) and tells what its navigation looks like (updateNavigation), and attaches its own
// listeners after calling super.
class MediaView
{
    constructor (model, uiElements) {
        this._model = model;
        this.uiElements = uiElements;

        this.currentVideoVolumeChangedEvent = new Event(this);
        this.firstNavButtonClickedEvent = new Event(this);
        this.previousNavButtonClickedEvent = new Event(this);
        this.nextNavButtonClickedEvent = new Event(this);
        this.lastNavButtonClickedEvent = new Event(this);
        this.goBackTenImagesPressedEvent = new Event(this);
        this.goForwardTenImagesPressedEvent = new Event(this);
        this.playButtonClickedEvent = new Event(this);
        this.pauseButtonClickedEvent = new Event(this);
        this.settingChangedEvent = new Event(this);

        this.isSettingVolume = false;
    }

    attachModelListeners()
    {
        var _this = this;

        this._model.videoVolumeUpdatedEvent.attach(function () {
            _this.updateVideoVolume();
            _this.updateVideoMuted();
        });

        this._model.currentSlideChangedEvent.attach(function () {
            _this.updateSlidesAndNavigation();
        });

        this._model.playingChangedEvent.attach(function () {
            _this.updatePlayPauseButtons();
        });

        this._model.settingUpdatedEvent.attach(function (key) {
            _this.updateSetting(key);
        });
    }

    attachUiElementListeners()
    {
        var _this = this;

        window.addEventListener('resize', function () {
            _this.windowResized();
        });

        this.uiElements.currentVideo.addEventListener('volumechange', function() {
            if (_this.isSettingVolume)
            {
                return;
            }

            _this.currentVideoVolumeChangedEvent.notify();
        });

        this.uiElements.firstNavButton.addEventListener('click', function() {
            _this.firstNavButtonClickedEvent.notify();
        });

        this.uiElements.previousNavButton.addEventListener('click', function() {
            _this.previousNavButtonClickedEvent.notify();
        });

        this.uiElements.nextNavButton.addEventListener('click', function() {
            _this.nextNavButtonClickedEvent.notify();
        });

        this.uiElements.lastNavButton.addEventListener('click', function() {
            _this.lastNavButtonClickedEvent.notify();
        });

        this.uiElements.playButton.addEventListener('click', function() {
            _this.playButtonClickedEvent.notify();
        });

        this.uiElements.pauseButton.addEventListener('click', function() {
            _this.pauseButtonClickedEvent.notify();
        });

        for (let setting of this._model.settings)
        {
            this.uiElements[setting.element].addEventListener('change', function () {
                _this.settingChangedEvent.notify(setting.key);
            });
        }
    }

    // The value in the control of a setting, as text (or checked or not).
    getSetting(key)
    {
        var control = this.uiElements[this._model.settingsByKey[key].element];

        return control.type == 'checkbox' ? control.checked : control.value.trim();
    }

    // Puts the model's value in the control.
    updateSetting(key)
    {
        var setting = this._model.settingsByKey[key];
        var control = this.uiElements[setting.element];
        var value = this._model[key];

        if (control.type == 'checkbox')
            control.checked = value;
        else
            control.value = value == null ? '' : String(value).trim();

        if (setting.after)
            setting.after(this);
    }

    updateOptions() {
        for (let key of ['secondsPerSlide', 'playVideosToEnd', 'maxWidth', 'maxHeight'])
            this.updateSetting(key);
    }

    enableOrDisableMaxWidthAndHeight() {
        var textBoxesDisabled = !this._model.areMaxWithAndHeightEnabled();

        this.uiElements.maxWidthTextBox.disabled = textBoxesDisabled;
        this.uiElements.maxHeightTextBox.disabled = textBoxesDisabled;
    }

    clearUI() {
        this.clearWarningMessage();
        this.clearInfoMessage();
        this.clearImage();
        this.clearVideo();
        this.hideNavigation();
        this.clearThumbnails();
    }

    windowResized() {
        if (this._model.autoFitSlide)
        {
            this.updateSlideSize();
        }
    }

    isDisplayingWarningMessage() {
        return this.uiElements.warningMessage.style.display == 'block';
    }

    displayWarningMessage(message) {
        this.uiElements.warningMessage.textContent = message;
        this.uiElements.warningMessage.style.display = 'block';
    }

    clearWarningMessage() {
        this.uiElements.warningMessage.textContent = '';
        this.uiElements.warningMessage.style.display = 'none';
    }

    displayInfoMessage(message) {
        this.uiElements.infoMessage.textContent = message;
        this.uiElements.infoMessage.style.display = 'block';
    }

    clearInfoMessage() {
        this.uiElements.infoMessage.textContent = '';
        this.uiElements.infoMessage.style.display = 'none';
    }

    updateSlidesAndNavigation() {
        this.updateSlides();
        this.updateNavigation();
    }

    updateSlides() {
        this.displayCurrentSlide();
        this.showThumbnails();
    }

    setupLoadingAnimation() {
        var _this = this;

        this.uiElements.currentImage.onload = function () {
            _this.hideLoadingAnimation();
        }

        this.uiElements.currentVideo.addEventListener('loadeddata', function() {
            _this.hideLoadingAnimation();
        }, false);
    }

    displaySlide() {
        this.showLoadingAnimation();

        var currentSlide = this._model.getCurrentSlide();

        if (currentSlide == null)
            return;

        if (currentSlide.isImageOrGif())
        {
            this.displayImage(currentSlide);
        }
        else if (currentSlide.isVideo())
        {
            this.displayVideo(currentSlide);
        }
        else
        {
            console.log("Trying to display slide that isn't an image or video.")
        }
    }

    displayImage(currentSlide) {
        var currentImage = this.uiElements.currentImage;

        logForDev('image = ' + currentSlide.fileUrl);

        currentImage.src = displayUrl(currentSlide.fileUrl);
        currentImage.setAttribute('alt', currentSlide.id);
        currentImage.style.display = 'inline';

        this.clearVideo();
        this.updateSlideSize();
    }

    displayVideo(currentSlide) {
        var currentVideo = this.uiElements.currentVideo;

        logForDev('video = ' + currentSlide.fileUrl);

        currentVideo.src = displayUrl(currentSlide.fileUrl);
        currentVideo.style.display = 'inline';

        this.clearImage();
        this.updateSlideSize();
        this.updateVideoVolume();
        this.updateVideoMuted();
    }

    getVideoVolume() {
        return this.uiElements.currentVideo.volume;
    }

    getVideoMuted() {
        return this.uiElements.currentVideo.muted;
    }

    updateSlideSize() {
        var currentSlide = this._model.getCurrentSlide();

        if (currentSlide == null)
            return;

        var currentImage = this.uiElements.currentImage;
        var currentVideo = this.uiElements.currentVideo;

        // The one that is shown gets the size, the other one none.
        var shown = currentSlide.isImageOrGif() ? currentImage : currentSlide.isVideo() ? currentVideo : null;

        for (let element of [currentImage, currentVideo])
        {
            element.style.width = null;
            element.style.height = null;
            element.style.maxWidth = null;
            element.style.maxHeight = null;
        }

        if (shown == null)
        {
            console.log("Couldn't update slide size because slide isn't image or video.");
            return;
        }

        if (this._model.autoFitSlide)
        {
            var viewWidth = window.innerWidth || document.documentElement.clientWidth || document.body.clientWidth;
            var viewHeight = window.innerHeight || document.documentElement.clientHeight || document.body.clientHeight;

            var newWidth = currentSlide.width;
            var newHeight = currentSlide.height;

            var viewRatio = viewWidth / viewHeight;
            var newRatio = newWidth / newHeight;

            if (newRatio > viewRatio)
            {
                newWidth = viewWidth;
                newHeight = viewWidth / newRatio;
            }
            else
            {
                newWidth = viewHeight * newRatio;
                newHeight = viewHeight;
            }

            shown.style.width = newWidth + 'px';
            shown.style.height = newHeight + 'px';
        }
        else
        {
            if (this._model.maxWidth != null)
                shown.style.maxWidth = parseInt(this._model.maxWidth) + 'px';

            if (this._model.maxHeight != null)
                shown.style.maxHeight = parseInt(this._model.maxHeight) + 'px';
        }
    }

    clearImage() {
        var currentImage = this.uiElements.currentImage;

        logForDev('image cleared');

        currentImage.src = '';
        currentImage.removeAttribute('alt');
        currentImage.style.display = 'none';
    }

    clearVideo() {
        var currentVideo = this.uiElements.currentVideo;

        currentVideo.src = '';
        currentVideo.style.display = 'none';
    }

    updateVideoVolume() {
        this.isSettingVolume = true;
        this.uiElements.currentVideo.volume = this._model.videoVolume;
        this.isSettingVolume = false;
    }

    updateVideoMuted() {
        this.uiElements.currentVideo.muted = this._model.videoMuted;
    }

    showLoadingAnimation() {
        this.uiElements.loadingAnimation.style.display = 'inline';
    }

    hideLoadingAnimation() {
        this.uiElements.loadingAnimation.style.display = 'none';
    }

    showNavigation() {
        this.uiElements.navigation.style.display = 'block';
    }

    hideNavigation() {
        this.uiElements.navigation.style.display = 'none';
    }

    updateNavigationButtonsAndDisplay() {
        this.updateCurrentNumberDisplay();
        this.updateTotalNumberDisplay();

        this.updateFirstPreviousButtons();
        this.updatePlayPauseButtons();
        this.updateNextLastButtons();
    }

    updateCurrentNumberDisplay() {
        this.uiElements.currentSlideNumber.textContent = this._model.getCurrentSlideNumber();
    }

    updateTotalNumberDisplay() {
        var totalNumberText = this._model.getSlideCount();

        if (this._model.areThereMoreLoadableSlides()) {
            totalNumberText += '+';
        }

        this.uiElements.totalSlideNumber.textContent = totalNumberText;
    }

    updateFirstPreviousButtons() {
        var currentlyOnFirstSlide = (this._model.getCurrentSlideNumber() == 1);

        this.uiElements.firstNavButton.disabled = currentlyOnFirstSlide;
        this.uiElements.previousNavButton.disabled = currentlyOnFirstSlide;
    }

    updatePlayPauseButtons() {
        if (this._model.isPlaying) {
            this.uiElements.playButton.style.display = 'none';
            this.uiElements.pauseButton.style.display = 'inline';
        }
        else {
            this.uiElements.playButton.style.display = 'inline';
            this.uiElements.pauseButton.style.display = 'none';
        }
    }

    updateNextLastButtons() {
        var currentlyOnLastSlide = (this._model.getCurrentSlideNumber() == this._model.getSlideCount());

        this.uiElements.nextNavButton.disabled = currentlyOnLastSlide;
        this.uiElements.lastNavButton.disabled = currentlyOnLastSlide;
    }

    showThumbnails() {
        this.clearThumbnails();

        var nextSlides = this._model.getNextSlidesForThumbnails();

        if (nextSlides == null)
            return;

        var _this = this;

        for (var i = 0; i < nextSlides.length; i++) {
            var slide = nextSlides[i];

            var showGreyedOut = !slide.isPreloaded
            this.displayThumbnail(thumbnailUrl(slide.previewFileUrl), slide.id, showGreyedOut);

            slide.clearCallback();
            slide.addCallback(function () {
                var callbackSlide = this;
                _this.removeThumbnailGreyness(callbackSlide.id);
                _this._model.preloadNextUnpreloadedSlideAfterThisOneIfInRange(callbackSlide);
            });
        }
    }

    displayThumbnail(thumbnailImageUrl, thumbnailSlideId, showGreyedOut) {
        var thumbnailList = this.uiElements.thumbnailList;

        var newThumbnail = document.createElement("div");
        newThumbnail.classList.add("thumbnail");
        newThumbnail.setAttribute('title', thumbnailSlideId);

        var _this = this;
        newThumbnail.onclick = function () {
            _this._model.moveToThumbnailSlide(thumbnailSlideId);
        };

        // A video from your own folders has no preview picture: show its first frame.
        var isLocalVideo = /^file:.*\.(mp4|webm|m4v|mov|ogv)$/i.test(thumbnailImageUrl);
        var newThumbnailImage = document.createElement(isLocalVideo ? "video" : "img");
        newThumbnailImage.id = 'thumbnail-image-' + thumbnailSlideId;
        newThumbnailImage.classList.add("thumbnail-image");
        if (isLocalVideo) {
            newThumbnailImage.muted = true;
            newThumbnailImage.preload = "metadata";
        }
        newThumbnailImage.src = isLocalVideo ? thumbnailImageUrl + '#t=0.1' : thumbnailImageUrl;

        if (showGreyedOut) {
            newThumbnailImage.classList.add("thumbnail-image-greyed-out");
        }

        newThumbnail.appendChild(newThumbnailImage);
        thumbnailList.appendChild(newThumbnail);
    }

    removeThumbnailGreyness(thumbnailSlideId) {
        var thumbnail = document.getElementById('thumbnail-image-' + thumbnailSlideId);

        if (thumbnail != null) {
            thumbnail.classList.remove('thumbnail-image-greyed-out');
        }
    }

    clearThumbnails() {
        this.uiElements.thumbnailList.replaceChildren();
    }

    // Lets the current play-through of the shown video finish instead of looping, then runs the callback.
    runWhenCurrentVideoEnds(callback) {
        this.cancelWaitingForVideoEnd();

        var currentVideo = this.uiElements.currentVideo;

        if (currentVideo.style.display == 'none' || currentVideo.paused || currentVideo.ended) {
            callback();
            return;
        }

        var _this = this;

        this.videoEndedListener = function () {
            _this.cancelWaitingForVideoEnd();
            callback();
        };

        currentVideo.loop = false;
        currentVideo.addEventListener('ended', this.videoEndedListener);
    }

    cancelWaitingForVideoEnd() {
        var currentVideo = this.uiElements.currentVideo;

        currentVideo.loop = true;

        if (this.videoEndedListener) {
            currentVideo.removeEventListener('ended', this.videoEndedListener);
            this.videoEndedListener = null;
        }
    }

    openUrlInNewWindow(url) {
        window.open(url, '_blank');
    }

    openCurrentSlideSource()
    {
        let currentSlide = this._model.getCurrentSlide();

        if (currentSlide == null)
            return;

        this.openUrlInNewWindow(currentSlide.viewableWebsitePostUrl);
    }

    downloadCurrentSlide()
    {
        let currentSlide = this._model.getCurrentSlide();

        if (currentSlide == null)
            return;

        let url = currentSlide.fileUrl;

        let filename = "MSG/" + url.substring(url.lastIndexOf('/')+1);

        chrome.downloads.download({
            url: currentSlide.fileUrl,
            filename: filename,
            conflictAction: "overwrite"
        });
    }
}

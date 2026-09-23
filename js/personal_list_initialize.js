let personalListController = null;

document.addEventListener('DOMContentLoaded', function () {
    personalListController = new PersonalListController({
        'warningMessage': document.getElementById('warning-message'),
        'infoMessage': document.getElementById('info-message'),
        'currentImage': document.getElementById('current-image'),
        'currentVideo': document.getElementById('current-video'),
        'currentVideoSourceWebm': document.getElementById('current-video-source-webm'),
        'loadingAnimation': document.getElementById('loading-animation'),
        'navigation': document.getElementById('navigation'),
        'currentSlideNumber': document.getElementById('current-slide-number'),
        'totalSlideNumber': document.getElementById('total-slide-number'),
        'thumbnailList': document.getElementById('thumbnail-list'),
        'filterTextBox': document.getElementById('filter-text'),
        'siteFilterSelect': document.getElementById('filter-site'),
        'filterButton': document.getElementById('filter-button'),
        'randomizeButton': document.getElementById('randomize-button'),
        'downloadFavoritesButton': document.getElementById('download-favorites-button'),
        'firstNavButton': document.getElementById('first-button'),
        'previousNavButton': document.getElementById('previous-button'),
        'nextNavButton': document.getElementById('next-button'),
        'lastNavButton': document.getElementById('last-button'),
        'playButton': document.getElementById('play-button'),
        'pauseButton': document.getElementById('pause-button'),
        'secondsPerSlideTextBox': document.getElementById('seconds-per-slide'),
        'maxWidthTextBox': document.getElementById('max-width'),
        'maxHeightTextBox': document.getElementById('max-height'),
        'playVideosToEndCheckBox': document.getElementById('play-videos-to-end'),
        'autoFitSlideCheckBox': document.getElementById('auto-fit-slide')
    });
});
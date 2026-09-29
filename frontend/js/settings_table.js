// The settings that are one field of the model with one control on the page (the control is uiElements[element]).
// The models, views and controllers of both pages handle them all the same way (js/mvc/media_*.js).
//   key    the field of the model, and its key in the storage
//   parse  turns the text of the control into the value to keep; undefined leaves the setting as it was
//   after  what the view does once the value is in the control
function positiveNumber(text)
{
    return text === '' || isNaN(text) || text < 1 ? undefined : text;
}

function optionalPositiveNumber(text)
{
    return text === '' ? null : positiveNumber(text);
}

// On both pages.
const SLIDE_SETTINGS = [
    { key: 'secondsPerSlide', element: 'secondsPerSlideTextBox', parse: positiveNumber },
    { key: 'maxWidth', element: 'maxWidthTextBox', parse: optionalPositiveNumber, after: view => view.updateSlideSize() },
    { key: 'maxHeight', element: 'maxHeightTextBox', parse: optionalPositiveNumber, after: view => view.updateSlideSize() },
    {
        key: 'autoFitSlide',
        element: 'autoFitSlideCheckBox',
        after: function (view) {
            view.enableOrDisableMaxWidthAndHeight();
            view.updateSlideSize();
        }
    },
    { key: 'playVideosToEnd', element: 'playVideosToEndCheckBox' }
];

// On the slideshow page only.
const SEARCH_SETTINGS = [
    { key: 'includeImages', element: 'includeImagesCheckBox' },
    { key: 'includeGifs', element: 'includeGifsCheckBox' },
    { key: 'includeWebms', element: 'includeWebmsCheckBox' },
    { key: 'includeExplicit', element: 'includeExplicitCheckBox' },
    { key: 'includeQuestionable', element: 'includeQuestionableCheckBox' },
    { key: 'includeSafe', element: 'includeSafeCheckBox' },
    { key: 'includeDupes', element: 'includeDupesCheckBox' },
    { key: 'hideBlacklist', element: 'hideBlacklist', after: view => view.hideOrShowBlacklist() },
    { key: 'blacklist', element: 'blacklist' },
    { key: 'derpibooruApiKey', element: 'derpibooruApiKey' },
    { key: 'tantabusApiKey', element: 'tantabusApiKey' },
    { key: 'e621Login', element: 'e621Login' },
    { key: 'e621ApiKey', element: 'e621ApiKey' },
    { key: 'gelbUserId', element: 'gelbUserId' },
    { key: 'gelbApiKey', element: 'gelbApiKey' },
    { key: 'storeHistory', element: 'storeHistoryCheckBox' }
];

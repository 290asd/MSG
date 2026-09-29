// Faves and unfaves a post on the site too, when it is faved or unfaved in the app.
// Only e621 so far; it needs the e621 username and API key from Settings → Sites & accounts,
// and can be turned off in Settings → Favorites.
class RemoteFavorites
{
    static async sync(slide, faved)
    {
        if (slide == null || slide.siteId != SITE_E621)
            return;

        let settings = await chrome.storage.sync.get(['e621Login', 'e621ApiKey', 'syncE621Favorites']);

        if (settings.syncE621Favorites === false)
            return;

        // Without the API key it stays a favorite in the app only.
        if (!settings.e621Login || !settings.e621ApiKey)
            return;

        let authorization = 'Basic ' + btoa(settings.e621Login + ':' + settings.e621ApiKey);

        try
        {
            let response = faved
                ? await fetch('https://e621.net/favorites.json', {
                    method: 'POST',
                    headers: { 'Authorization': authorization, 'Content-Type': 'application/x-www-form-urlencoded' },
                    body: 'post_id=' + encodeURIComponent(slide.id)
                })
                : await fetch('https://e621.net/favorites/' + encodeURIComponent(slide.id) + '.json', {
                    method: 'DELETE',
                    headers: { 'Authorization': authorization }
                });

            // Success needs no notice (the heart effect shows the fave). 422 (already a favorite)
            // and 404 (wasn't one) mean e621 is already as wanted. Only failures are told.
            if (response.ok || response.status == 422 || (!faved && response.status == 404))
                return;
            else if (response.status == 401 || response.status == 403)
                RemoteFavorites.notify('e621 didn\'t accept the username and API key (' + response.status + '). Check them in Settings → Sites & accounts.');
            else
                RemoteFavorites.notify('Couldn\'t ' + (faved ? 'fave' : 'unfave') + ' on e621 (error ' + response.status + ').');
        }
        catch (e)
        {
            RemoteFavorites.notify('Couldn\'t reach e621 to ' + (faved ? 'fave' : 'unfave') + ' the post.');
        }
    }

    // Shown by app_settings.js as a short notice.
    static notify(message)
    {
        window.dispatchEvent(new CustomEvent('app-notice', { detail: message }));
    }
}

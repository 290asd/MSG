// Imports the user's favorites from every site that has login details saved in the settings.
// The site managers parse the results, so imported items look the same as ones added with the ♥ button.
class FavoritesImporter
{
    constructor(personalList)
    {
        this.personalList = personalList;
    }

    async getCredentials()
    {
        return chrome.storage.sync.get(['derpibooruApiKey', 'e621Login', 'e621ApiKey', 'gelbUserId', 'gelbApiKey']);
    }

    // Model for the site managers that lets every post through: favorites ignore the search filters.
    buildSitesManager(credentials)
    {
        return {
            model: Object.assign({
                includeImages: true,
                includeGifs: true,
                includeWebms: true,
                includeExplicit: false,
                includeQuestionable: false,
                includeSafe: false,
                areSomeTagsAreBlacklisted: () => false
            }, credentials),
            displayWarningMessage: (message) => console.log(message)
        };
    }

    getSources(credentials, sitesManager)
    {
        let sources = [];

        if (credentials.e621Login)
            sources.push({ name: 'e621', query: 'fav:' + credentials.e621Login, siteManager: new SiteManagerE621(sitesManager, 320) });

        if (credentials.derpibooruApiKey)
            sources.push({ name: 'Derpibooru', query: 'my:faves', siteManager: new SiteManagerDerpibooru(sitesManager, 50) });

        if (credentials.gelbUserId && credentials.gelbApiKey)
            sources.push({ name: 'Gelbooru', query: 'fav:' + credentials.gelbUserId, siteManager: new SiteManagerGelbooru(sitesManager, 100) });

        return sources;
    }

    // Returns [{name, found, added}] or throws if no site has login details.
    async importFavorites(onProgress)
    {
        let credentials = await this.getCredentials();
        let sources = this.getSources(credentials, this.buildSitesManager(credentials));

        if (sources.length == 0)
            throw new Error('No site has login details saved. Add your e621 username, Derpibooru API key or Gelbooru user ID and API key on the slideshow page.');

        let results = [];

        for (let source of sources)
        {
            let slides = await this.fetchAllPages(source, onProgress);
            let countBefore = this.personalList.count();

            // Sites list favorites newest first; the personal list keeps the newest last.
            for (let slide of slides.reverse())
                this.personalList.tryToAdd(slide);

            results.push({ name: source.name, found: slides.length, added: this.personalList.count() - countBefore });
        }

        return results;
    }

    async fetchAllPages(source, onProgress)
    {
        let siteManager = source.siteManager;

        for (let page = 1; !siteManager.hasExhaustedSearch; page++)
        {
            onProgress(source.name + ': loading page ' + page + ' (' + siteManager.allUnsortedSlides.length + ' favorites so far)');

            let response = await fetch(siteManager.buildRequestUrl(source.query, page));

            if (!response.ok)
                throw new Error(source.name + ' returned ' + response.status + '. Check your login details.');

            siteManager.addSlides(await response.text());

            // Stay under the sites' rate limits (e621 allows about 2 requests per second).
            await new Promise(resolve => setTimeout(resolve, 1000));
        }

        return siteManager.allUnsortedSlides;
    }
}

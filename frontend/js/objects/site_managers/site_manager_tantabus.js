// Tantabus is a Philomena booru like Derpibooru: same API and JSON, see SiteManagerDerpibooru.
class SiteManagerTantabus extends SiteManagerDerpibooru
{
    constructor(sitesManager, pageLimit)
    {
		super(sitesManager, pageLimit, SITE_TANTABUS, 'https://www.tantabus.ai');
		this.jsonPostsKey = 'images';
		this.apiKeyName = 'tantabusApiKey';
    }

    buildPingRequestUrl()
	{
		return this.url + '/api/v1/json/search/images?q=pony&per_page=1';
    }
}

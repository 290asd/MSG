class SiteManagerGelbooru extends SiteManager
{
    constructor(sitesManager, pageLimit)
    {
		super(sitesManager, SITE_GELBOORU, 'https://gelbooru.com', pageLimit);
		this.xmlPostsTag = 'posts';
    }

	buildUserIdAndApiKeyString()
	{
		return this.sitesManager.model.gelbApiKey && this.sitesManager.model.gelbUserId ? '&user_id=' + this.sitesManager.model.gelbUserId + '&api_key=' + this.sitesManager.model.gelbApiKey : '';
	}
    
    buildPingRequestUrl()
	{
		return this.url + '/index.php?page=dapi&s=post&q=index&limit=1' + this.buildUserIdAndApiKeyString();
    }
    
    buildRequestUrl(searchText, pageNumber)
	{
		var query = this.buildSiteSpecificQuery(searchText);
		return this.url + '/index.php?page=dapi&s=post&q=index&tags=' + query + '&pid=' + (pageNumber - 1) + '&limit=' + this.pageLimit + this.buildUserIdAndApiKeyString();
	}

	addSlide(xmlPost)
	{
		
		if (doesXmlContainElement(xmlPost, 'file_url') &&
			doesXmlContainElement(xmlPost, 'preview_url') &&
			this.isPathForSupportedMediaType(getXmlElementStringValueSafe(xmlPost, 'file_url')))
		{
			if (this.areSomeTagsAreBlacklisted(getXmlElementStringValueSafe(xmlPost, 'tags')))
				return;
			
			var newSlide = new Slide(
				SITE_GELBOORU,
				getXmlElementStringValueSafe(xmlPost, 'id'),
				this.reformatFileUrl(getXmlElementStringValueSafe(xmlPost, 'file_url')),
				this.reformatFileUrl(getXmlElementStringValueSafe(xmlPost, 'preview_url')),
				this.url + '/index.php?page=post&s=view&id=' + getXmlElementStringValueSafe(xmlPost, 'id'),
				getXmlElementStringValueSafe(xmlPost, 'width'),
				getXmlElementStringValueSafe(xmlPost, 'height'),
				new Date(getXmlElementStringValueSafe(xmlPost, 'created_at')),
				getXmlElementStringValueSafe(xmlPost, 'score'),
				this.getMediaTypeFromPath(getXmlElementStringValueSafe(xmlPost, 'file_url')),
				getXmlElementStringValueSafe(xmlPost, 'md5'),
				getXmlElementStringValueSafe(xmlPost, 'tags')
			);

			this.allUnsortedSlides.push(newSlide);
		}
	}
}
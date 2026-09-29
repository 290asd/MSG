class SiteManagerRule34 extends SiteManager
{
    constructor(sitesManager, pageLimit)
    {
		super(sitesManager, SITE_RULE34, 'https://rule34.xxx', pageLimit);
    }
    
    // Rule34's API moved to api.rule34.xxx and needs a user ID and API key
    // (rule34.xxx → My Account → Options → API Access Credentials).
    buildApiUrl()
	{
		let model = this.sitesManager.model;
		let credentials = model.rule34UserId && model.rule34ApiKey
			? '&user_id=' + encodeURIComponent(model.rule34UserId) + '&api_key=' + encodeURIComponent(model.rule34ApiKey)
			: '';

		return 'https://api.rule34.xxx/index.php?page=dapi&s=post&q=index' + credentials;
	}

    buildPingRequestUrl()
	{
		return this.buildApiUrl() + '&limit=1';
    }
    
    buildRequestUrl(searchText, pageNumber)
	{
		var query = this.buildSiteSpecificQuery(searchText);
		
		return this.buildApiUrl() + '&tags=' + query + '&pid=' + (pageNumber - 1) + '&limit=' + this.pageLimit;
	}

	// Without a user ID and API key the API answers with an <error>: the site is up, it just needs them.
	doesResponseTextIndicateOnline(responseText)
	{
		var parser = new DOMParser();
		var xml = parser.parseFromString(responseText, "text/xml");
		
		return xml.getElementsByTagName("post").length > 0 || xml.getElementsByTagName("error").length > 0;
	}

	addSlides(responseText)
	{
		var error = new DOMParser().parseFromString(responseText, "text/xml").getElementsByTagName("error")[0];

		if (error)
		{
			this.ranIntoErrorWhileSearching = true;
			this.sitesManager.displayWarningMessage('Rule34: ' + error.textContent.trim().replace(/\.?$/, '.') + ' Add your Rule34 user ID and API key in Settings → Sites & accounts.');
			return;
		}

		this.addXmlSlides(responseText);
	}

	addSlide(xmlPost)
	{
		if (xmlPost.hasAttribute('file_url') &&
			xmlPost.hasAttribute('preview_url'))
		{
			if (this.isPathForSupportedMediaType(xmlPost.getAttribute('file_url')))
			{
				if (this.areSomeTagsAreBlacklisted(xmlPost.getAttribute('tags')))
					return;
				
				if (!this.isRatingAllowed(xmlPost.getAttribute('rating')))
					return;

				var newSlide = new Slide(
					SITE_RULE34,
					xmlPost.getAttribute('id'),
					this.reformatFileUrl(xmlPost.getAttribute('file_url')),
					this.reformatFileUrl(xmlPost.getAttribute('preview_url')),
					this.url + '/index.php?page=post&s=view&id=' + xmlPost.getAttribute('id'),
					xmlPost.getAttribute('width'),
					xmlPost.getAttribute('height'),
					new Date(xmlPost.getAttribute('created_at')),
					xmlPost.getAttribute('score'),
					this.getMediaTypeFromPath(xmlPost.getAttribute('file_url')),
					xmlPost.getAttribute('md5'),
					xmlPost.getAttribute('tags')
				);
				
				this.allUnsortedSlides.push(newSlide);
			}
		}
	}
}
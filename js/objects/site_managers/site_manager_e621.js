class SiteManagerE621 extends SiteManager
{
    constructor(sitesManager, pageLimit)
    {
		super(sitesManager, SITE_E621, 'https://e621.net', pageLimit);
    }
    
    buildPingRequestUrl()
	{
		return this.url + '/posts.json?limit=1';
    }
    
    buildRequestUrl(searchText, pageNumber)
	{
		var query = this.buildSiteSpecificQuery(searchText);
		return this.url + '/posts.json?tags=' + query + '&page=' + pageNumber + '&limit=' + this.pageLimit + this.loginParameters();
	}

	loginParameters()
	{
		let model = this.sitesManager.model;
		return model.e621ApiKey && model.e621Login ? '&login=' + encodeURIComponent(model.e621Login) + '&api_key=' + encodeURIComponent(model.e621ApiKey) : '';
	}

	resetConnection()
	{
		super.resetConnection();
		this.pool = null;
	}

	// A pool: its post IDs in page order first, then the posts a page at a time ("id:1,2,3"),
	// sorted back into that order.
	performSearch(searchText, doneSearchingSiteCallback)
	{
		let poolId = poolIdFromSearch(searchText);

		if (poolId == null)
			return super.performSearch(searchText, doneSearchingSiteCallback);

		this.ranIntoErrorWhileSearching = false;

		if (this.pool && this.pool.id == poolId)
			return this.loadPoolPage(doneSearchingSiteCallback);

		this.webRequester.makeWebsiteRequest(
			this.url + '/pools/' + poolId + '.json' + this.loginParameters().replace('&', '?'),
			(responseText) => {
				let pool;
				try {
					pool = JSON.parse(responseText);
				} catch (e) {
					this.ranIntoErrorWhileSearching = true;
					return;
				}
				this.pool = { id: pool.id, name: String(pool.name).replace(/_/g, ' '), postIds: pool.post_ids || [] };
				window.dispatchEvent(new CustomEvent('pool-loaded', { detail: { id: this.pool.id, name: this.pool.name, count: this.pool.postIds.length } }));
			},
			this.handleErrorFromSiteResponse.bind(this),
			() => {
				if (this.pool)
					this.loadPoolPage(doneSearchingSiteCallback);
				else
					doneSearchingSiteCallback(this);
			},
			(url, hide) => {
				this.handleGeneralError(url, hide);
				doneSearchingSiteCallback(this);
			});
	}

	loadPoolPage(doneSearchingSiteCallback)
	{
		let size = Math.min(this.pageLimit, 100);
		let ids = this.pool.postIds.slice(this.lastPageLoaded * size, (this.lastPageLoaded + 1) * size);

		if (ids.length == 0)
		{
			this.hasExhaustedSearch = true;
			doneSearchingSiteCallback(this);
			return;
		}

		let order = new Map(this.pool.postIds.map((id, i) => [id, i]));

		this.webRequester.makeWebsiteRequest(
			this.url + '/posts.json?tags=id:' + ids.join(',') + '&limit=' + size + this.loginParameters(),
			(responseText) => {
				this.lastPageLoaded++;
				this.addSlides(responseText);
				this.allUnsortedSlides.sort((a, b) => order.get(a.id) - order.get(b.id));
				this.hasExhaustedSearch = this.lastPageLoaded * size >= this.pool.postIds.length;
			},
			this.handleErrorFromSiteResponse.bind(this),
			() => doneSearchingSiteCallback(this),
			(url, hide) => {
				this.ranIntoErrorWhileSearching = true;
				this.handleGeneralError(url, hide);
				doneSearchingSiteCallback(this);
			});
	}

	doesResponseTextIndicateOnline(responseText)
	{
		var jsonPosts;
		
		try
		{
			jsonPosts = JSON.parse(responseText);
		}
		catch(e)
		{
			console.log("JSON failed to parse.");
			console.log(e);
			return false;
		}
		
		if (jsonPosts == null)
			return false;
		
		return (jsonPosts.posts.length > 0);
	}

	addSlides(responseText)
	{
		this.addJsonSlides(responseText);
	}

	condenseTags(jsonPostTags)
	{
		var condensedTagArray = [];

		for(var prop in jsonPostTags)
		{
			condensedTagArray = condensedTagArray.concat(jsonPostTags[prop]);
		}

		return condensedTagArray.join(" ");
	}

	addSlide(jsonPost)
	{
		if (!jsonPost.hasOwnProperty('id') ||
			!jsonPost.hasOwnProperty('file') ||
			!jsonPost.hasOwnProperty('preview') ||
			!jsonPost.hasOwnProperty('created_at') ||
			!jsonPost.hasOwnProperty('score') ||
			!jsonPost.hasOwnProperty('tags') ||
			!jsonPost.file.hasOwnProperty('url'))
			return;

		if (jsonPost.file.url == null)
		{
			// TODO: Make this into an option (though will need to figure out all the cases in which it happens to get a proper name for it)
			console.log("The file.url was null for post " + jsonPost.id + ". (This is caused by a global blacklist perhaps from not being logged in. Trying to recreate from md5.");
			
			jsonPost.file.url = this.RecreateUrlFromMd5(jsonPost.file);

			if (jsonPost.file.url == null)
			{
				return;
			}
			else
			{
				console.log("New URL is: " + jsonPost.file.url);
			}
		}
		
		if (!this.isPathForSupportedMediaType(jsonPost.file.url))
			return;
		
		if (!this.isRatingAllowed(jsonPost.rating))
			return;

		// Kept by category for the tag panel (artist, character, species, …).
		var tagGroups = jsonPost.tags;

		jsonPost.tags = this.condenseTags(jsonPost.tags);
		
		if (this.areSomeTagsAreBlacklisted(jsonPost.tags))
			return;
		
		var postUrl = this.url + '/post/show/' + jsonPost.id;
		
		var urlPrefix = '';
		
		if (postUrl.substring(0, 4) != 'http')
			urlPrefix = 'https://';

		logForDev(urlPrefix + jsonPost.file.url + " ; " + urlPrefix + jsonPost.preview.url + " ; " + postUrl)
		
		var newSlide = new Slide(
			SITE_E621,
			jsonPost.id,
			urlPrefix + jsonPost.file.url,
			urlPrefix + jsonPost.preview.url,
			postUrl,
			jsonPost.file.width,
			jsonPost.file.height,
			new Date(jsonPost.created_at),
			jsonPost.score,
			this.getMediaTypeFromPath(jsonPost.file.url),
			jsonPost.file.md5,
			jsonPost.tags
		);

		newSlide.tagGroups = tagGroups;

		if (jsonPost.sample && jsonPost.sample.url)
			newSlide.sampleFileUrl = urlPrefix + jsonPost.sample.url;

		this.allUnsortedSlides.push(newSlide);
	}

	RecreateUrlFromMd5(file)
	{
		// Ref: Discord > e621.net > Donovan DMC
		// Take the file.md5 (as an example: AABBCCCCCCCCCCCCCCCCCCCCCCCCCCCC)
		// and convert it to
		// https://static1.e621.net/data/AA/BB/AABBCCCCCCCCCCCCCCCCCCCCCCCCCCCC.{file.ext}
		
		const BASE_PATH = 'https://static1.e621.net/data/';

		if (file.md5 == null || file.md5.length < 4 || file.ext == null)
		{
			return null;
		}

		let firstTwoCharacters = file.md5.substring(0, 2);
		let secondTwoCharacters = file.md5.substring(2, 4);

		return `${BASE_PATH}/${firstTwoCharacters}/${secondTwoCharacters}/${file.md5}.${file.ext}`
	}
}
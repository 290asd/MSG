// Realbooru's API is switched off ("shut off indefinitely"), so this reads the site's own pages: the
// search list gives id, md5 and tags; the file's extension, score and date are only on each post's page.
class SiteManagerRealbooru extends SiteManager
{
    constructor(sitesManager, pageLimit)
    {
		super(sitesManager, SITE_REALBOORU, 'https://realbooru.com', pageLimit);
		this.searchGeneration = 0;
    }

	buildListUrl(tags, pageNumber)
	{
		return this.url + '/index.php?page=post&s=list&tags=' + tags + '&pid=' + (pageNumber - 1) * this.pageLimit;
	}

	buildPingRequestUrl()
	{
		return this.buildListUrl('all', 1);
	}

	buildRequestUrl(searchText, pageNumber)
	{
		var tags = this.buildSiteSpecificQuery(searchText).split(/\s+/).filter(Boolean).map(encodeURIComponent).join('+');

		return this.buildListUrl(tags || 'all', pageNumber);
	}

	doesResponseTextIndicateOnline(responseText)
	{
		return responseText.includes('thumbnail_');
	}

	resetConnection()
	{
		super.resetConnection();
		this.searchGeneration++; // posts still being read for the old search are dropped
	}

	// The slides are added after the posts' pages have been read, so "done" is told only then.
	performSearch(searchText, doneSearchingSiteCallback)
	{
		if (!this.isOnline)
			return;

		this.ranIntoErrorWhileSearching = false;

		var siteManager = this;
		var generation = this.searchGeneration;
		var pending = null;

		this.webRequester.makeWebsiteRequest(
			this.buildRequestUrl(searchText, this.lastPageLoaded + 1),
			function(responseText){
				siteManager.lastPageLoaded++;
				pending = siteManager.addPosts(responseText, generation);
			},
			this.handleErrorFromSiteResponse.bind(this),
			function(){
				Promise.resolve(pending).then(function(){
					if (generation == siteManager.searchGeneration)
						doneSearchingSiteCallback(siteManager);
				});
			},
			function(url, hideVisibleWarning){
				siteManager.ranIntoErrorWhileSearching = true;
				siteManager.handleGeneralError(url, hideVisibleWarning);
				doneSearchingSiteCallback(siteManager);
			}
		);
	}

	async addPosts(listHtml, generation)
	{
		var posts = [];

		for (var thumb of new DOMParser().parseFromString(listHtml, 'text/html').querySelectorAll('div.thumb'))
		{
			var image = thumb.querySelector('img');
			var md5 = image && (image.src.match(/thumbnail_([0-9a-f]{32})/) || [])[1];
			var id = (thumb.id || '').replace(/^s/, '');

			if (!md5 || !/^\d+$/.test(id))
				continue;

			// The list shows tags as "big ass, dark skin"; the rest of the app uses "big_ass dark_skin".
			var tags = image.title.split(', ').map(function(tag){ return tag.trim().replace(/\s+/g, '_'); }).join(' ');

			posts.push({ id: id, md5: md5, tags: tags, previewUrl: image.src });
		}

		this.hasExhaustedSearch = (posts.length < this.pageLimit);

		posts = posts.filter(function(post){ return !this.areSomeTagsAreBlacklisted(post.tags); }, this);

		// Two posts' pages at a time: more and the site answers 503 (see readPost).
		var slides = new Array(posts.length);
		var next = 0;
		var siteManager = this;

		var worker = async function()
		{
			while (next < posts.length)
			{
				var index = next++;

				try
				{
					slides[index] = await siteManager.readPost(posts[index]);
				}
				catch (e)
				{
					// This post is left out; the rest of the page still shows.
				}
			}
		};

		await Promise.all([worker(), worker()]);

		if (generation != this.searchGeneration)
			return;

		for (var slide of slides)
		{
			if (slide)
				this.allUnsortedSlides.push(slide);
		}
	}

	async readPost(post)
	{
		var response;

		// The site rations requests with a 503; a short wait and another try gets through.
		for (var attempt = 1; attempt <= 5; attempt++)
		{
			response = await fetch(this.url + '/index.php?page=post&s=view&id=' + post.id);

			if (response.status != 503)
				break;

			await new Promise(function(resolve){ setTimeout(resolve, 600 * attempt); });
		}

		if (!response.ok)
			return null;

		var page = new DOMParser().parseFromString(await response.text(), 'text/html');
		var media = page.querySelector('#image, video source');
		var fileUrl = media && media.getAttribute('src');

		if (!fileUrl || !this.isPathForSupportedMediaType(fileUrl))
			return null;

		var score = page.getElementById('psc' + post.id);
		var posted = (page.body.textContent.match(/Posted at ([A-Za-z]+,? \d+ \d{4})/) || [])[1];

		// No width and height here: they are taken from the file once it has loaded (setupLocalFileSizes).
		return new Slide(
			SITE_REALBOORU,
			post.id,
			this.reformatFileUrl(fileUrl),
			post.previewUrl,
			this.url + '/index.php?page=post&s=view&id=' + post.id,
			0,
			0,
			new Date(posted || 0),
			score ? Number(score.textContent) || 0 : 0,
			this.getMediaTypeFromPath(fileUrl),
			post.md5,
			post.tags
		);
	}
}

// Reads a model's settings from the storage when the page opens, and saves them when they change.
class DataLoader
{
    constructor(model)
    {
        this._model = model;
        this.loading = false;
    }

    async loadUserSettings()
    {
        let stored = await chrome.storage.sync.get(this._model.storageKeys());

        if (stored == null)
        {
            return;
        }

        // What was just read is not written back.
        this.loading = true;

        try
        {
            this._model.applyStoredSettings(stored);
        }
        finally
        {
            this.loading = false;
        }
    }

    // The model's field of that name, saved under the same name.
    save(key)
    {
        if (!this.loading)
        {
            chrome.storage.sync.set({[key]: this._model[key]});
        }
    }

    savePersonalList(items)
    {
        if (!this.loading)
        {
            chrome.storage.local.set({'personalListItems': items ? items : this._model.personalList.personalListItems});
        }
    }
}

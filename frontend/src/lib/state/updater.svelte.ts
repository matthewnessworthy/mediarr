import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

class UpdaterState {
	/** Whether an update is available */
	available = $state(false);
	/** Version string of the available update */
	version = $state<string | null>(null);
	/** Release notes body (markdown) */
	body = $state<string | null>(null);
	/** Whether we are currently checking for updates */
	checking = $state(false);
	/** Whether we are downloading/installing */
	downloading = $state(false);
	/** Download progress in bytes */
	downloaded = $state(0);
	/** Total download size in bytes (0 if unknown) */
	totalSize = $state(0);
	/** Error message from last check or download attempt */
	error = $state<string | null>(null);

	/** Cached update handle from check() */
	private _update: Update | null = null;

	/** Check for available updates. Returns true if update found. */
	async checkForUpdates(): Promise<boolean> {
		this.checking = true;
		this.error = null;
		try {
			const update = await check();
			if (update) {
				this._update = update;
				this.available = true;
				this.version = update.version;
				this.body = update.body ?? null;
				return true;
			}
			this.available = false;
			this.version = null;
			this.body = null;
			this._update = null;
			return false;
		} catch (e) {
			this.error = e instanceof Error ? e.message : String(e);
			return false;
		} finally {
			this.checking = false;
		}
	}

	/** Download and install the available update, then relaunch. */
	async downloadAndInstall(): Promise<void> {
		if (!this._update) return;
		this.downloading = true;
		this.downloaded = 0;
		this.totalSize = 0;
		this.error = null;
		try {
			await this._update.downloadAndInstall((event) => {
				if (event.event === 'Started') {
					this.totalSize = event.data.contentLength ?? 0;
					this.downloaded = 0;
				} else if (event.event === 'Progress') {
					this.downloaded += event.data.chunkLength;
				} else if (event.event === 'Finished') {
					this.downloading = false;
				}
			});
			await relaunch();
		} catch (e) {
			this.error = e instanceof Error ? e.message : String(e);
			this.downloading = false;
		}
	}

	/** Computed progress percentage (0-100). */
	get progress(): number {
		if (this.totalSize === 0) return 0;
		return Math.min(100, Math.round((this.downloaded / this.totalSize) * 100));
	}
}

export const updaterState = new UpdaterState();

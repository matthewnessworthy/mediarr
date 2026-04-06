import type { Page } from '@playwright/test';

/**
 * Inline implementation of Tauri's mockIPC, mockWindows, and clearMocks.
 * Extracted from @tauri-apps/api/mocks so it can be injected via addInitScript
 * (which runs before any page modules load, ensuring IPC calls on mount are caught).
 */
const TAURI_MOCK_SCRIPT = `
(function() {
  window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ || {};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = window.__TAURI_EVENT_PLUGIN_INTERNALS__ || {};

  // Mock window metadata (equivalent to mockWindows('main'))
  window.__TAURI_INTERNALS__.metadata = {
    currentWindow: { label: 'main' },
    currentWebview: { windowLabel: 'main', label: 'main' },
  };

  // Storage for mock handlers and IPC data
  window.__PLAYWRIGHT_MOCK_HANDLERS__ = {};

  const callbacks = new Map();

  function registerCallback(callback, once) {
    const id = Math.floor(Math.random() * 0xFFFFFFFF);
    callbacks.set(id, function(data) {
      if (once) callbacks.delete(id);
      return callback && callback(data);
    });
    return id;
  }

  function unregisterCallback(id) {
    callbacks.delete(id);
  }

  function runCallback(id, data) {
    const cb = callbacks.get(id);
    if (cb) cb(data);
  }

  // Event mocking
  const eventListeners = new Map();

  async function invoke(cmd, args, _options) {
    // Handle event plugin commands
    if (cmd === 'plugin:event|listen') {
      if (!eventListeners.has(args.event)) eventListeners.set(args.event, []);
      eventListeners.get(args.event).push(args.handler);
      return args.handler;
    }
    if (cmd === 'plugin:event|emit') {
      const listeners = eventListeners.get(args.event) || [];
      for (const handler of listeners) {
        const cb = callbacks.get(handler);
        if (cb) cb(args);
      }
      return null;
    }
    if (cmd === 'plugin:event|unlisten') {
      const listeners = eventListeners.get(args.event);
      if (listeners) {
        const idx = listeners.indexOf(args.id);
        if (idx !== -1) listeners.splice(idx, 1);
      }
      return;
    }

    // Look up mock handler
    const h = window.__PLAYWRIGHT_MOCK_HANDLERS__;
    if (cmd in h) {
      return h[cmd];
    }
    console.warn('[mock] Unmocked IPC command: ' + cmd);
    return null;
  }

  window.__TAURI_INTERNALS__.invoke = invoke;
  window.__TAURI_INTERNALS__.transformCallback = registerCallback;
  window.__TAURI_INTERNALS__.unregisterCallback = unregisterCallback;
  window.__TAURI_INTERNALS__.runCallback = runCallback;
  window.__TAURI_INTERNALS__.callbacks = callbacks;
  window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener = function(event, id) {
    unregisterCallback(id);
  };
})();
`;

/**
 * Set up IPC mock handlers for a Playwright page.
 * Handlers is a record of command name -> return value.
 * Values are serialized as JSON so they must be plain objects (no functions).
 */
export async function setupMocks(page: Page, handlers: Record<string, unknown>) {
	await page.evaluate((h) => {
		(window as any).__PLAYWRIGHT_MOCK_HANDLERS__ = h;
	}, handlers);
}

/**
 * Default mock handlers that make all pages render without errors.
 * Returns plain serializable objects (no functions -- they can't cross page.evaluate boundary).
 */
export function defaultMockData(): Record<string, unknown> {
	return {
		get_config: {
			general: {
				output_dir: '/output',
				operation: 'Move',
				conflict_strategy: 'Skip',
				create_directories: true,
			},
			templates: {
				movie: '{title} ({year})/{title} ({year}).{ext}',
				series: '{title}/Season {season:02}/{title} - S{season:02}E{episode:02}.{ext}',
			},
			subtitles: {
				enabled: true,
				naming_pattern: '{title}.{language}.{type}.{ext}',
				discovery: {
					sidecar: true,
					subs_subfolder: true,
					nested_language_folders: true,
					vobsub_pairs: true,
				},
				preferred_languages: ['eng'],
				non_preferred_action: 'Ignore',
				backup_path: null,
			},
			watchers: [],
		},
		update_config: null,
		scan_folder: [],
		scan_folder_streaming: null,
		dry_run_renames: [],
		execute_renames: [],
		preview_template: 'Preview Output',
		validate_template: [],
		list_batches: [],
		check_undo: { eligible: true, batch_id: 'test', ineligible_reasons: [] },
		execute_undo: [],
		list_watchers: [],
		list_watcher_events: [],
		list_review_queue: [],
		start_watcher: null,
		stop_watcher: null,
	};
}

/**
 * Navigate to a page with default mocks already set up.
 * Uses addInitScript to inject Tauri IPC mocks before any page JS runs,
 * ensuring IPC calls in onMount are intercepted.
 */
export async function gotoWithMocks(
	page: Page,
	path: string,
	overrides: Record<string, unknown> = {},
) {
	const handlers = { ...defaultMockData(), ...overrides };

	// Inject the Tauri mock infrastructure before page loads
	await page.addInitScript(TAURI_MOCK_SCRIPT);

	// Inject mock handler data before page loads
	await page.addInitScript((h: Record<string, unknown>) => {
		(window as any).__PLAYWRIGHT_MOCK_HANDLERS__ = h;
	}, handlers);

	// Navigate -- all IPC calls on mount will be intercepted
	await page.goto(path, { waitUntil: 'networkidle' });
}

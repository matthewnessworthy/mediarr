import { afterEach } from 'vitest';
import { clearMocks } from '@tauri-apps/api/mocks';
import { randomFillSync } from 'node:crypto';

// happy-dom doesn't provide WebCrypto -- Tauri mocks need it
if (typeof globalThis.crypto === 'undefined') {
	Object.defineProperty(globalThis, 'crypto', {
		value: {
			getRandomValues: (buffer: Uint8Array) => randomFillSync(buffer),
		},
	});
}

afterEach(() => {
	clearMocks();
});

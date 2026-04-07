import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

export type WithElementRef<T, El extends HTMLElement = HTMLElement> = T & {
	ref?: El | null;
};

export type WithoutChildrenOrChild<T> = Omit<T, 'children' | 'child'>;

/** Extract the filename from a path (handles both / and \ separators). */
export function basename(path: string): string {
	return path.split(/[\\/]/).pop() ?? path;
}

/** Truncate a long path for display, keeping first and last segments. */
export function truncatePath(path: string, maxLen = 50): string {
	if (path.length <= maxLen) return path;
	const parts = path.split('/');
	if (parts.length <= 3) return '...' + path.slice(-(maxLen - 3));
	return parts[0] + '/.../' + parts.slice(-2).join('/');
}

/** Format an ISO timestamp as a human-readable relative time string. */
export function relativeTime(iso: string): string {
	const diff = Date.now() - new Date(iso).getTime();
	const seconds = Math.floor(diff / 1000);
	if (seconds < 60) return 'just now';
	const minutes = Math.floor(seconds / 60);
	if (minutes < 60) return `${minutes} min ago`;
	const hours = Math.floor(minutes / 60);
	if (hours < 24) return `${hours} hour${hours === 1 ? '' : 's'} ago`;
	const days = Math.floor(hours / 24);
	return `${days} day${days === 1 ? '' : 's'} ago`;
}

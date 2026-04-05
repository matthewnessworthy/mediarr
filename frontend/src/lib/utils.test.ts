import { describe, it, expect } from 'vitest';
import { cn } from './utils';

describe('cn', () => {
	it('merges tailwind classes correctly (last wins)', () => {
		expect(cn('p-4', 'p-2')).toBe('p-2');
	});

	it('handles conditional classes', () => {
		expect(cn('base', false && 'hidden')).toBe('base');
	});

	it('combines non-conflicting classes', () => {
		expect(cn('p-4', 'mt-2')).toBe('p-4 mt-2');
	});

	it('handles empty inputs', () => {
		expect(cn()).toBe('');
	});

	it('handles undefined and null values', () => {
		expect(cn('base', undefined, null, 'extra')).toBe('base extra');
	});
});

import { describe, expect, it } from 'vitest';
import { getParentFolder } from '$lib/utils/tauri';

describe('getParentFolder', () => {
  it('handles POSIX paths', () => {
    expect(getParentFolder('/usr/local/bin')).toBe('/usr/local');
    expect(getParentFolder('/usr/local/bin/')).toBe('/usr/local');
    expect(getParentFolder('/')).toBe('/');
  });

  it('handles Windows paths', () => {
    expect(getParentFolder('C:\\Users\\Alice\\Documents')).toBe('C:\\Users\\Alice');
    expect(getParentFolder('C:\\Users\\Alice\\Documents\\')).toBe('C:\\Users\\Alice');
    expect(getParentFolder('C:\\Users')).toBe('C:\\');
    expect(getParentFolder('C:\\')).toBe('C:\\');
  });

  it('handles mixed separators', () => {
    expect(getParentFolder('C:/Users/Alice/Documents')).toBe('C:/Users/Alice');
    expect(getParentFolder('folder/subfolder/file')).toBe('folder/subfolder');
    expect(getParentFolder('filename-only')).toBe('filename-only');
  });
});

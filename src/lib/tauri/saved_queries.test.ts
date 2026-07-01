import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
  listFolders,
  createFolder,
  updateFolder,
  deleteFolder,
  listSavedQueries,
  createSavedQuery,
  updateSavedQuery,
  deleteSavedQuery,
} from './saved_queries';

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockInvoke.mockReset();
});

describe('listFolders', () => {
  it('invokes saved_queries_list_folders with no arguments', async () => {
    mockInvoke.mockResolvedValue([]);
    await listFolders();
    expect(mockInvoke).toHaveBeenCalledWith('saved_queries_list_folders');
  });

  it('returns the value from invoke', async () => {
    const folders = [{ id: 'f1', name: 'My Folder', parentId: null, position: 0 }];
    mockInvoke.mockResolvedValue(folders);
    const result = await listFolders();
    expect(result).toBe(folders);
  });
});

describe('createFolder', () => {
  it('invokes saved_queries_create_folder with input', async () => {
    const input = { name: 'New Folder' };
    mockInvoke.mockResolvedValue({ id: 'f1', name: 'New Folder', parentId: null, position: 0 });
    await createFolder(input);
    expect(mockInvoke).toHaveBeenCalledWith('saved_queries_create_folder', { input });
  });

  it('passes parentId and position when provided', async () => {
    const input = { name: 'Sub Folder', parentId: 'parent-1', position: 2 };
    mockInvoke.mockResolvedValue({
      id: 'f2',
      name: 'Sub Folder',
      parentId: 'parent-1',
      position: 2,
    });
    await createFolder(input);
    expect(mockInvoke).toHaveBeenCalledWith('saved_queries_create_folder', { input });
  });
});

describe('updateFolder', () => {
  it('invokes saved_queries_update_folder with id and input', async () => {
    const input = { name: 'Renamed Folder' };
    mockInvoke.mockResolvedValue({ id: 'f1', name: 'Renamed Folder', parentId: null, position: 0 });
    await updateFolder('f1', input);
    expect(mockInvoke).toHaveBeenCalledWith('saved_queries_update_folder', { id: 'f1', input });
  });
});

describe('deleteFolder', () => {
  it('invokes saved_queries_delete_folder with id', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await deleteFolder('f1');
    expect(mockInvoke).toHaveBeenCalledWith('saved_queries_delete_folder', { id: 'f1' });
  });
});

describe('listSavedQueries', () => {
  it('invokes saved_queries_list with null folderId when called with no argument', async () => {
    mockInvoke.mockResolvedValue([]);
    await listSavedQueries();
    expect(mockInvoke).toHaveBeenCalledWith('saved_queries_list', { folderId: null });
  });

  it('invokes saved_queries_list with null folderId when called with null', async () => {
    mockInvoke.mockResolvedValue([]);
    await listSavedQueries(null);
    expect(mockInvoke).toHaveBeenCalledWith('saved_queries_list', { folderId: null });
  });

  it('invokes saved_queries_list with the provided folderId', async () => {
    mockInvoke.mockResolvedValue([]);
    await listSavedQueries('folder-1');
    expect(mockInvoke).toHaveBeenCalledWith('saved_queries_list', { folderId: 'folder-1' });
  });

  it('returns the value from invoke', async () => {
    const queries = [
      {
        id: 'q1',
        name: 'My Query',
        sql: 'SELECT 1',
        folderId: null,
        connectionId: null,
        createdAt: '',
        updatedAt: '',
      },
    ];
    mockInvoke.mockResolvedValue(queries);
    const result = await listSavedQueries();
    expect(result).toBe(queries);
  });
});

describe('createSavedQuery', () => {
  it('invokes saved_queries_create with input', async () => {
    const input = { name: 'New Query', sql: 'SELECT 1' };
    mockInvoke.mockResolvedValue({
      id: 'q1',
      name: 'New Query',
      sql: 'SELECT 1',
      folderId: null,
      connectionId: null,
      createdAt: '',
      updatedAt: '',
    });
    await createSavedQuery(input);
    expect(mockInvoke).toHaveBeenCalledWith('saved_queries_create', { input });
  });

  it('passes connectionId and folderId when provided', async () => {
    const input = { name: 'Linked', sql: 'SELECT 2', connectionId: 'conn-1', folderId: 'f1' };
    mockInvoke.mockResolvedValue({ id: 'q2', ...input, createdAt: '', updatedAt: '' });
    await createSavedQuery(input);
    expect(mockInvoke).toHaveBeenCalledWith('saved_queries_create', { input });
  });
});

describe('updateSavedQuery', () => {
  it('invokes saved_queries_update with id and input', async () => {
    const input = { name: 'Updated', sql: 'SELECT 3' };
    mockInvoke.mockResolvedValue({
      id: 'q1',
      name: 'Updated',
      sql: 'SELECT 3',
      folderId: null,
      connectionId: null,
      createdAt: '',
      updatedAt: '',
    });
    await updateSavedQuery('q1', input);
    expect(mockInvoke).toHaveBeenCalledWith('saved_queries_update', { id: 'q1', input });
  });
});

describe('deleteSavedQuery', () => {
  it('invokes saved_queries_delete with id', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await deleteSavedQuery('q1');
    expect(mockInvoke).toHaveBeenCalledWith('saved_queries_delete', { id: 'q1' });
  });
});

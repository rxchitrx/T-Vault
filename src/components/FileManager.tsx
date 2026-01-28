import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/api/dialog';
import { createDir } from '@tauri-apps/api/fs';
import {
  Upload,
  FolderPlus,
  Download,
  Trash2,
  File,
  Folder,
  ChevronRight,
  Search,
  Check,
  X,
} from 'lucide-react';
import { FileListSkeleton } from './SkeletonLoader';
import DeleteConfirmationModal from './DeleteConfirmationModal';

interface FileMetadata {
  id: string;
  name: string;
  size: number;
  mime_type: string;
  created_at: number;
  folder: string;
  is_folder: boolean;
  thumbnail?: string;
}

interface FolderStats {
  file_count: number;
  total_size: number;
}

interface DownloadItem {
  id: string;
  name: string;
  size: number;
  destination: string;
}

interface ToastFunctions {
  showSuccess: (message: string, duration?: number) => void;
  showError: (message: string, duration?: number) => void;
  showInfo: (message: string, duration?: number) => void;
  showWarning: (message: string, duration?: number) => void;
}

interface FileManagerProps {
  currentFolder: string;
  onFolderChange: (folder: string) => void;
  toast?: ToastFunctions;
  onUploadTrigger?: (filePaths: string[], targetFolder: string) => void;
  onDownloadTrigger?: (files: DownloadItem[]) => void;
}

// Row height constant for index-based selection
const ROW_HEIGHT = 66; // Approximate height of each file row in pixels
const LIST_PADDING_TOP = 24; // py-6 = 24px

export default function FileManager({
  currentFolder,
  onFolderChange,
  toast,
  onUploadTrigger,
  onDownloadTrigger,
}: FileManagerProps) {
  const [files, setFiles] = useState<FileMetadata[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [showFolderDialog, setShowFolderDialog] = useState(false);
  const [newFolderName, setNewFolderName] = useState('');
  
  const [deleteTargets, setDeleteTargets] = useState<FileMetadata[]>([]);
  const [isDeleting, setIsDeleting] = useState(false);
  const [shouldAnimate, setShouldAnimate] = useState(true);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set());
  const [newFileIds, setNewFileIds] = useState<Set<string>>(() => new Set()); // Track newly added files for animation
  const knownFileIdsRef = useRef<Set<string>>(new Set()); // Track all known file IDs
  
  // Lasso state - minimal, just for visual
  const [lassoBox, setLassoBox] = useState<{ startY: number; endY: number } | null>(null);
  const lassoStartYRef = useRef<number | null>(null);
  const isDraggingRef = useRef(false);
  const fileListContainerRef = useRef<HTMLDivElement | null>(null);
  
  // Drag and drop state
  const [isDragOver, setIsDragOver] = useState(false);
  
  const [folderToDownload, setFolderToDownload] = useState<FileMetadata | null>(null);
  const [folderStats, setFolderStats] = useState<FolderStats | null>(null);
  const [isGettingStats, setIsGettingStats] = useState(false);
  const refreshTimerRef = useRef<NodeJS.Timeout | null>(null);

  const filteredFiles = files.filter((file) =>
    file.name.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const loadFiles = async (showSkeleton = true, animate = true) => {
    if (showSkeleton) {
      setIsLoading(true);
    }
    
    try {
      const fileList = await invoke<FileMetadata[]>('list_files', {
        folder: currentFolder,
      });
      
      if (animate) {
        // Full refresh - animate all items, reset known IDs
        setShouldAnimate(true);
        setNewFileIds(new Set());
        knownFileIdsRef.current = new Set(fileList.map(f => f.id));
        setFiles(fileList);
      } else {
        // Background refresh - only animate new files
        const newIds = fileList
          .filter(f => !knownFileIdsRef.current.has(f.id))
          .map(f => f.id);
        
        if (newIds.length > 0 || fileList.length !== files.length) {
          // Add new IDs to known set
          newIds.forEach(id => knownFileIdsRef.current.add(id));
          setNewFileIds(new Set(newIds));
          setShouldAnimate(false); // Don't animate all, just the new ones
          setFiles(fileList);
        }
        // If no changes, don't update state at all
      }
    } catch (error) {
      console.error('Failed to load files:', error);
    } finally {
      if (showSkeleton) {
        setIsLoading(false);
      }
    }
  };

  // Load files only when folder changes
  useEffect(() => {
    loadFiles(true, true);
    // Reset known files when changing folders
    knownFileIdsRef.current = new Set();
  }, [currentFolder]);

  // Setup event listeners separately (use refs to avoid re-subscribing)
  const onUploadTriggerRef = useRef(onUploadTrigger);
  const toastRef = useRef(toast);
  const currentFolderRef = useRef(currentFolder);
  
  useEffect(() => {
    onUploadTriggerRef.current = onUploadTrigger;
    toastRef.current = toast;
    currentFolderRef.current = currentFolder;
  });

  useEffect(() => {
    let unlistenUpload: (() => void) | null = null;
    let unlistenFileDrop: (() => void) | null = null;
    let unlistenFileDropHover: (() => void) | null = null;
    let unlistenFileDropCancelled: (() => void) | null = null;
    
    const setupListeners = async () => {
      try {
        // Upload progress listener
        unlistenUpload = await listen('upload-progress', (event: any) => {
          const data = event.payload as any;
          if (data.status === 'completed' && data.folder === currentFolderRef.current) {
            if (refreshTimerRef.current) clearTimeout(refreshTimerRef.current);
            refreshTimerRef.current = setTimeout(() => {
              loadFiles(false, false);
              refreshTimerRef.current = null;
            }, 1000); 
          }
        });

        // File drop listener (Tauri native)
        unlistenFileDrop = await listen('tauri://file-drop', (event: any) => {
          const paths = event.payload as string[];
          if (paths && paths.length > 0 && onUploadTriggerRef.current) {
            onUploadTriggerRef.current(paths, currentFolderRef.current);
            toastRef.current?.showInfo(`Adding ${paths.length} file(s) to upload queue`, 2000);
          }
          setIsDragOver(false);
        });

        // File drop hover listener
        unlistenFileDropHover = await listen('tauri://file-drop-hover', () => {
          setIsDragOver(true);
        });

        // File drop cancelled listener
        unlistenFileDropCancelled = await listen('tauri://file-drop-cancelled', () => {
          setIsDragOver(false);
        });

      } catch (error) {
        console.error('Failed to setup listeners:', error);
      }
    };
    
    setupListeners();
    return () => { 
      if (unlistenUpload) unlistenUpload(); 
      if (unlistenFileDrop) unlistenFileDrop();
      if (unlistenFileDropHover) unlistenFileDropHover();
      if (unlistenFileDropCancelled) unlistenFileDropCancelled();
      if (refreshTimerRef.current) clearTimeout(refreshTimerRef.current);
    };
  }, []); // Empty deps - only run once on mount

  useEffect(() => {
    setSelectedIds(new Set());
  }, [currentFolder]);

  useEffect(() => {
    // Sync selected IDs with current files (remove selections for deleted files)
    const currentFileIds = new Set(files.map(f => f.id));
    setSelectedIds((prev) => {
      if (prev.size === 0) return prev;
      const validIds = [...prev].filter(id => currentFileIds.has(id));
      if (validIds.length === prev.size) return prev; // No change needed
      return new Set(validIds);
    });
  }, [files]);

  // Convert Y position to file index
  const yToIndex = useCallback((y: number) => {
    const adjustedY = y - LIST_PADDING_TOP;
    if (adjustedY < 0) return 0;
    return Math.floor(adjustedY / ROW_HEIGHT);
  }, []);

  // Get selected indices from lasso box
  const getSelectedIndicesFromLasso = useCallback((startY: number, endY: number) => {
    const minY = Math.min(startY, endY);
    const maxY = Math.max(startY, endY);
    
    const startIndex = yToIndex(minY);
    const endIndex = yToIndex(maxY);
    
    const indices: number[] = [];
    for (let i = startIndex; i <= endIndex && i < filteredFiles.length; i++) {
      if (i >= 0) indices.push(i);
    }
    return indices;
  }, [yToIndex, filteredFiles.length]);

  // Lasso handlers - super simple, no DOM queries during drag
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement;
    if (target.closest('button') || target.closest('input')) return;
    
    const container = fileListContainerRef.current;
    if (!container) return;
    
    const rect = container.getBoundingClientRect();
    const y = e.clientY - rect.top + container.scrollTop;
    
    lassoStartYRef.current = y;
    isDraggingRef.current = false;
  }, []);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (lassoStartYRef.current === null) return;
    
    const container = fileListContainerRef.current;
    if (!container) return;
    
    const rect = container.getBoundingClientRect();
    const currentY = e.clientY - rect.top + container.scrollTop;
    const startY = lassoStartYRef.current;
    
    // Only activate drag if moved more than 5px
    if (Math.abs(currentY - startY) > 5) {
      isDraggingRef.current = true;
      setLassoBox({ startY, endY: currentY });
      
      // Calculate selection based on indices - no DOM queries!
      const indices = getSelectedIndicesFromLasso(startY, currentY);
      const newSelectedIds = new Set(indices.map(i => filteredFiles[i]?.id).filter(Boolean) as string[]);
      setSelectedIds(newSelectedIds);
    }
  }, [filteredFiles, getSelectedIndicesFromLasso]);

  const handleMouseUp = useCallback(() => {
    if (lassoStartYRef.current !== null && !isDraggingRef.current) {
      // It was a click, not a drag - clear selection
      setSelectedIds(new Set());
    }
    lassoStartYRef.current = null;
    isDraggingRef.current = false;
    setLassoBox(null);
  }, []);

  // Global mouse up
  useEffect(() => {
    const onMouseUp = () => handleMouseUp();
    window.addEventListener('mouseup', onMouseUp);
    return () => window.removeEventListener('mouseup', onMouseUp);
  }, [handleMouseUp]);

  const handleUpload = async () => {
    try {
      const selected = await open({
        multiple: true,
        directory: false,
      });

      if (selected && onUploadTrigger) {
        const filePaths = Array.isArray(selected) ? selected : [selected];
        onUploadTrigger(filePaths, currentFolder);
      }
    } catch (error) {
      console.error('Upload failed:', error);
    }
  };

  const handleCreateFolder = () => {
    setNewFolderName('');
    setShowFolderDialog(true);
  };

  const handleFolderDialogSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    const trimmedName = newFolderName.trim();
    
    if (!trimmedName) {
      alert('Folder name cannot be empty');
      return;
    }
    
    setShowFolderDialog(false);
    
    try {
      await invoke('create_folder', {
        folderName: trimmedName,
        parentFolder: currentFolder,
      });
      
      toast?.showSuccess(`Folder "${trimmedName}" created`, 2000);
      await loadFiles(false);
    } catch (error) {
      console.error('Failed to create folder:', error);
      toast?.showError(`Failed to create folder: ${error}`, 3000);
    }
  };

  const handleFolderDialogCancel = () => {
    setShowFolderDialog(false);
    setNewFolderName('');
  };

  const buildFolderPath = (folderName: string) => {
    return currentFolder === '/' ? `/${folderName}` : `${currentFolder}/${folderName}`;
  };

  const handleDelete = (fileId: string) => {
    const file = files.find(f => f.id === fileId);
    if (file) {
      setDeleteTargets([file]);
    }
  };

  const handleBulkDelete = () => {
    const targets = files.filter((f) => selectedIds.has(f.id));
    if (targets.length > 0) {
      setDeleteTargets(targets);
    }
  };

  const confirmDelete = async () => {
    if (deleteTargets.length === 0) return;
    
    setIsDeleting(true);
    try {
      const results = await Promise.allSettled(
        deleteTargets.map(async (target) => {
          if (target.is_folder) {
            const folderPath = buildFolderPath(target.name);
            await invoke('delete_folder', { folderPath });
          } else {
            await invoke('delete_file', { fileId: target.id });
          }
        })
      );

      const failures = results.filter((result) => result.status === 'rejected');
      const successCount = results.length - failures.length;

      if (successCount > 0) {
        if (deleteTargets.length === 1 && failures.length === 0) {
          toast?.showSuccess(`"${deleteTargets[0].name}" deleted`, 2000);
        } else {
          toast?.showSuccess(`${successCount} items deleted`, 2000);
        }
      }

      if (failures.length > 0) {
        toast?.showError(`${failures.length} items failed to delete`, 3000);
      }

      loadFiles(false);
      setSelectedIds(new Set());
    } catch (error) {
      console.error('Failed to delete item:', error);
      toast?.showError(`Failed to delete: ${error}`, 3000);
    } finally {
      setIsDeleting(false);
      setDeleteTargets([]);
    }
  };

  const cancelDelete = () => {
    setDeleteTargets([]);
  };

  const getFolderDownloadItems = async (folder: FileMetadata, baseDir: string): Promise<DownloadItem[]> => {
    const folderPath = buildFolderPath(folder.name);
    const allFiles = await invoke<FileMetadata[]>('list_files_recursive', { folderPath });

    return allFiles.map((file) => {
      const relativeFolder = file.folder.startsWith(folderPath)
        ? file.folder.slice(folderPath.length)
        : '';
      const trimmedFolder = relativeFolder.replace(/^\/+/, '');
      const destinationDir = trimmedFolder
        ? `${baseDir}/${folder.name}/${trimmedFolder}`
        : `${baseDir}/${folder.name}`;

      return {
        id: file.id,
        name: file.name,
        size: file.size,
        destination: `${destinationDir}/${file.name}`,
      };
    });
  };

  const ensureDownloadDirectories = async (items: DownloadItem[]) => {
    const directories = new Set<string>();

    items.forEach((item) => {
      const lastSlash = item.destination.lastIndexOf('/');
      if (lastSlash > 0) {
        directories.add(item.destination.slice(0, lastSlash));
      }
    });

    await Promise.all(
      Array.from(directories).map((dir) => createDir(dir, { recursive: true }))
    );
  };

  const handleFolderDownloadRequest = async (folder: FileMetadata) => {
    setFolderToDownload(folder);
    setIsGettingStats(true);
    try {
      const folderPath = buildFolderPath(folder.name);
      const stats = await invoke<FolderStats>('get_folder_stats', { folderPath });
      setFolderStats(stats);
    } catch (error) {
      console.error('Failed to get folder stats:', error);
      toast?.showError('Failed to calculate folder size');
      setFolderToDownload(null);
    } finally {
      setIsGettingStats(false);
    }
  };

  const confirmFolderDownload = async () => {
    if (!folderToDownload || !onDownloadTrigger) return;
    
    const folderName = folderToDownload.name;
    
    setFolderToDownload(null);
    setFolderStats(null);

    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: `Select destination for "${folderName}"`
      });

      if (selected && !Array.isArray(selected)) {
        const downloadItems = await getFolderDownloadItems(folderToDownload, selected);

        if (downloadItems.length === 0) {
          toast?.showWarning('Folder is empty');
          return;
        }

        await ensureDownloadDirectories(downloadItems);
        onDownloadTrigger(downloadItems);
      }
    } catch (error) {
      console.error('Failed to queue folder download:', error);
      toast?.showError(`Folder download failed: ${error}`);
    }
  };

  const handleDownload = async (fileId: string, fileName: string, fileSize: number) => {
    if (!onDownloadTrigger) return;

    try {
      const ext = fileName.includes('.') ? fileName.split('.').pop() || '' : '';
      const savePath = await save({
        defaultPath: fileName,
        filters: ext ? [{
          name: ext.toUpperCase() + ' Files',
          extensions: [ext]
        }] : undefined
      });

      if (savePath) {
        onDownloadTrigger([{
          id: fileId,
          name: fileName,
          size: fileSize,
          destination: savePath
        }]);
      }
    } catch (error) {
      console.error('Failed to queue download:', error);
      toast?.showError(`Download failed: ${error}`);
    }
  };

  const handleBulkDownload = async () => {
    if (!onDownloadTrigger || selectedIds.size === 0) return;

    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: `Select destination folder for ${selectedIds.size} items`
      });

      if (!selected || Array.isArray(selected)) return;

      const selectedFiles = files.filter((f) => selectedIds.has(f.id));
      const allDownloadItems: DownloadItem[] = [];

      for (const file of selectedFiles) {
        if (file.is_folder) {
          const folderItems = await getFolderDownloadItems(file, selected);
          allDownloadItems.push(...folderItems);
        } else {
          allDownloadItems.push({
            id: file.id,
            name: file.name,
            size: file.size,
            destination: `${selected}/${file.name}`,
          });
        }
      }

      if (allDownloadItems.length === 0) {
        toast?.showWarning('No files to download');
        return;
      }

      await ensureDownloadDirectories(allDownloadItems);
      onDownloadTrigger(allDownloadItems);
      toast?.showInfo(`Added ${allDownloadItems.length} files to download queue`, 2000);
      setSelectedIds(new Set());
    } catch (error) {
      console.error('Failed to queue bulk download:', error);
      toast?.showError(`Bulk download failed: ${error}`);
    }
  };

  const handleFileClick = (file: FileMetadata) => {
    if (isDraggingRef.current) return;
    
    if (file.is_folder) {
      onFolderChange(`${currentFolder}/${file.name}`.replace('//', '/'));
    }
  };

  const toggleSelection = (fileId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(fileId)) {
        next.delete(fileId);
      } else {
        next.add(fileId);
      }
      return next;
    });
  };

  const selectAll = () => {
    setSelectedIds(new Set(filteredFiles.map((f) => f.id)));
  };

  const clearSelection = () => {
    setSelectedIds(new Set());
  };

  const formatFileSize = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
  };

  const formatDate = (timestamp: number): string => {
    return new Date(timestamp * 1000).toLocaleDateString();
  };

  const selectedCount = selectedIds.size;

  // Calculate lasso visual box position
  const lassoVisualStyle = lassoBox ? {
    top: Math.min(lassoBox.startY, lassoBox.endY),
    height: Math.abs(lassoBox.endY - lassoBox.startY),
  } : null;

  return (
    <div className="h-full flex flex-col relative">
      {/* Drag and Drop Overlay */}
      {isDragOver && (
        <div className="absolute inset-0 bg-blue-500/10 backdrop-blur-sm z-50 flex items-center justify-center pointer-events-none animate-fadeIn">
          <div className="bg-white dark:bg-dark-surface rounded-3xl p-12 shadow-large dark:shadow-large-dark border-2 border-dashed border-blue-400 text-center">
            <div className="w-20 h-20 bg-blue-100 dark:bg-blue-900/30 rounded-full flex items-center justify-center mx-auto mb-6">
              <Upload className="w-10 h-10 text-blue-600 dark:text-blue-400" />
            </div>
            <h2 className="text-2xl font-bold text-gray-900 dark:text-white mb-2">Drop files to upload</h2>
            <p className="text-gray-500 dark:text-zinc-500">Release to upload to {currentFolder === '/' ? 'root' : currentFolder}</p>
          </div>
        </div>
      )}

      {showFolderDialog && (
        <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50 animate-fadeIn">
          <div className="bg-white dark:bg-dark-surface rounded-2xl p-8 w-full max-w-md mx-4 shadow-large dark:shadow-large-dark animate-scaleIn">
            <h2 className="text-lg font-semibold mb-6 text-gray-900 dark:text-white">Create New Folder</h2>
            <form onSubmit={handleFolderDialogSubmit}>
              <input
                type="text"
                value={newFolderName}
                onChange={(e) => setNewFolderName(e.target.value)}
                placeholder="Folder name"
                className="input w-full mb-6"
                autoFocus
              />
              <div className="flex justify-end space-x-2.5">
                <button
                  type="button"
                  onClick={handleFolderDialogCancel}
                  className="btn btn-ghost"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  className="btn btn-primary"
                >
                  Create
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      <DeleteConfirmationModal
        isOpen={deleteTargets.length > 0}
        itemNames={deleteTargets.map((f) => f.name)}
        onConfirm={confirmDelete}
        onCancel={cancelDelete}
        isDeleting={isDeleting}
      />

      {folderToDownload && (
        <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50 animate-fadeIn">
          <div className="bg-white dark:bg-dark-surface rounded-2xl p-8 w-full max-w-md mx-4 shadow-large dark:shadow-large-dark animate-scaleIn text-center">
            <div className="w-16 h-16 bg-gray-100 dark:bg-zinc-800 rounded-full flex items-center justify-center mx-auto mb-6">
              <Download className="w-8 h-8 text-gray-900 dark:text-white" />
            </div>
            <h2 className="text-xl font-bold text-gray-900 dark:text-white mb-2">Download Folder?</h2>
            <p className="text-sm text-gray-500 dark:text-zinc-500 mb-6 px-4">
              Are you sure you want to download <span className="font-bold text-gray-900 dark:text-white">"{folderToDownload.name}"</span>?
            </p>

            <div className="bg-gray-50 dark:bg-zinc-900/50 rounded-2xl p-4 mb-8 flex justify-around">
              <div className="text-center">
                <p className="text-[10px] font-bold text-gray-400 dark:text-zinc-600 uppercase tracking-widest mb-1">Items</p>
                <p className="text-lg font-bold text-gray-900 dark:text-white">{isGettingStats ? '...' : folderStats?.file_count || 0}</p>
              </div>
              <div className="w-px h-10 bg-gray-200 dark:bg-zinc-700 my-auto" />
              <div className="text-center">
                <p className="text-[10px] font-bold text-gray-400 dark:text-zinc-600 uppercase tracking-widest mb-1">Total Size</p>
                <p className="text-lg font-bold text-gray-900 dark:text-white">{isGettingStats ? '...' : formatFileSize(folderStats?.total_size || 0)}</p>
              </div>
            </div>

            <div className="flex space-x-3">
              <button
                onClick={() => { setFolderToDownload(null); setFolderStats(null); }}
                className="flex-1 btn btn-secondary py-3"
              >
                Cancel
              </button>
              <button
                onClick={confirmFolderDownload}
                disabled={isGettingStats}
                className={`flex-1 btn btn-primary py-3 ${isGettingStats ? 'opacity-50 cursor-not-allowed' : ''}`}
              >
                Download All
              </button>
            </div>
          </div>
        </div>
      )}
      
      <div className="bg-white dark:bg-dark-surface border-b border-gray-100 dark:border-dark-border px-8 py-4">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center space-x-1.5 text-sm">
            <button
              onClick={() => onFolderChange('/')}
              className="text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white font-medium transition-colors"
            >
              Home
            </button>
            {currentFolder !== '/' &&
              currentFolder.split('/').filter(Boolean).map((part, index, arr) => (
                <div key={index} className="flex items-center space-x-1.5">
                  <ChevronRight className="w-3.5 h-3.5 text-gray-400 dark:text-zinc-600" />
                  <button
                    onClick={() =>
                      onFolderChange('/' + arr.slice(0, index + 1).join('/'))
                    }
                    className="text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white font-medium transition-colors"
                  >
                    {part}
                  </button>
                </div>
              ))}
          </div>

          <div className="flex items-center space-x-2">
            <button
              onClick={handleUpload}
              className="btn btn-primary ripple-effect group relative overflow-hidden"
            >
              <Upload className="w-4 h-4 mr-2 transition-transform duration-200 group-hover:scale-110" />
              Upload
            </button>
            <button
              onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                handleCreateFolder();
              }}
              className="btn btn-secondary ripple-effect group relative overflow-hidden"
              type="button"
            >
              <FolderPlus className="w-4 h-4 mr-2 transition-transform duration-200 group-hover:scale-110" />
              New Folder
            </button>
          </div>
        </div>

        <div className="relative group">
          <Search className="absolute left-3.5 top-1/2 transform -translate-y-1/2 w-4 h-4 text-gray-400 dark:text-zinc-600 transition-colors duration-200 group-focus-within:text-gray-600 dark:group-focus-within:text-zinc-500" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search files..."
            className="input pl-10 transition-all duration-200 focus:shadow-soft dark:focus:shadow-soft-dark focus:scale-[1.01]"
          />
        </div>
      </div>

      {/* Bulk Action Bar */}
      {selectedCount > 0 && (
        <div className="bg-gray-900 dark:bg-gray-800 text-white dark:text-gray-100 px-8 py-3 flex items-center justify-between animate-slideUp">
          <div className="flex items-center space-x-4">
            <span className="text-sm font-medium">
              {selectedCount} {selectedCount === 1 ? 'item' : 'items'} selected
            </span>
            <button
              onClick={selectAll}
              className="text-xs text-gray-300 dark:text-zinc-500 hover:text-white dark:hover:text-gray-300 transition-colors"
            >
              Select all ({filteredFiles.length})
            </button>
          </div>
          <div className="flex items-center space-x-2">
            <button
              onClick={handleBulkDownload}
              className="flex items-center space-x-1.5 px-3 py-1.5 bg-white/10 hover:bg-white/20 dark:bg-white/5 dark:hover:bg-white/10 rounded-lg transition-colors text-sm"
            >
              <Download className="w-4 h-4" />
              <span>Download</span>
            </button>
            <button
              onClick={handleBulkDelete}
              className="flex items-center space-x-1.5 px-3 py-1.5 bg-red-500/80 hover:bg-red-500 dark:bg-red-600/80 dark:hover:bg-red-600 rounded-lg transition-colors text-sm"
            >
              <Trash2 className="w-4 h-4" />
              <span>Delete</span>
            </button>
            <button
              onClick={clearSelection}
              className="p-1.5 hover:bg-white/10 dark:hover:bg-white/5 rounded-lg transition-colors ml-2"
              title="Clear selection"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      <div 
        ref={fileListContainerRef}
        className="flex-1 overflow-auto px-8 py-6 relative select-none"
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
      >
        {/* Lasso selection box - simple vertical bar */}
        {lassoVisualStyle && (
          <div
            className="absolute left-4 right-4 border-2 border-blue-400 bg-blue-400/10 pointer-events-none z-10 rounded"
            style={{
              top: lassoVisualStyle.top,
              height: lassoVisualStyle.height,
            }}
          />
        )}

        {isLoading ? (
          <FileListSkeleton />
        ) : filteredFiles.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-gray-400 dark:text-zinc-600 animate-fadeIn">
            <div className="w-16 h-16 bg-gray-100 dark:bg-zinc-800 rounded-2xl flex items-center justify-center mb-4 animate-bounce-subtle">
              <Folder className="w-8 h-8 text-gray-300 dark:text-zinc-600" />
            </div>
            <p className="text-base font-semibold text-gray-600 dark:text-gray-400 mb-1">No files yet</p>
            <p className="text-sm text-gray-400 dark:text-zinc-600">Upload your first file to get started</p>
          </div>
        ) : (
          <div className="space-y-1.5">
            {filteredFiles.map((file, index) => {
              const isSelected = selectedIds.has(file.id);
              // Animate if: full refresh (shouldAnimate=true) OR this is a newly added file
              const shouldAnimateItem = shouldAnimate || newFileIds.has(file.id);
              return (
                <div
                  key={file.id}
                  className={`card-hover p-4 cursor-pointer group ${shouldAnimateItem ? 'animate-fadeIn' : ''} ${
                    isSelected ? 'ring-2 ring-gray-900 dark:ring-white bg-gray-50 dark:bg-zinc-800/50' : ''
                  }`}
                  style={shouldAnimateItem ? { animationDelay: `${index * 0.03}s`, animationFillMode: 'both' } : {}}
                  onClick={() => handleFileClick(file)}
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center space-x-3.5 flex-1 min-w-0">
                      {/* Selection checkbox */}
                      <button
                        onClick={(e) => toggleSelection(file.id, e)}
                        className={`flex-shrink-0 w-5 h-5 rounded border-2 flex items-center justify-center transition-all duration-200 ${
                          isSelected
                            ? 'bg-gray-900 dark:bg-white border-gray-900 dark:border-white text-white dark:text-gray-900'
                            : 'border-gray-300 dark:border-zinc-600 hover:border-gray-400 dark:hover:border-zinc-500 group-hover:opacity-100 opacity-0'
                        }`}
                      >
                        {isSelected && <Check className="w-3 h-3" />}
                      </button>

                      <div className={`flex-shrink-0 w-10 h-10 rounded-xl flex items-center justify-center transition-all duration-300 group-hover:scale-110 ${
                        file.is_folder ? 'bg-gray-900 dark:bg-white' : 'bg-gray-100 dark:bg-zinc-800'
                      }`}>
                        {file.is_folder ? (
                          <Folder className="w-5 h-5 text-white dark:text-gray-900 transition-transform duration-300 group-hover:scale-110" />
                        ) : (
                          <File className="w-5 h-5 text-gray-500 dark:text-zinc-400 transition-transform duration-300 group-hover:scale-110" />
                        )}
                      </div>
                      <div className="flex-1 min-w-0">
                        <h3 className="font-medium text-gray-900 dark:text-white truncate text-sm">
                          {file.name}
                        </h3>
                        <p className="text-xs text-gray-400 dark:text-zinc-500 mt-0.5">
                          {file.is_folder
                            ? 'Folder'
                            : `${formatFileSize(file.size)} • ${formatDate(
                                file.created_at
                              )}`}
                        </p>
                      </div>
                    </div>

                    <div className="flex items-center space-x-1 opacity-0 group-hover:opacity-100 transition-all duration-300 transform translate-x-2 group-hover:translate-x-0">
                      {file.is_folder ? (
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            handleFolderDownloadRequest(file);
                          }}
                          className="p-2 hover:bg-gray-100 dark:hover:bg-zinc-800 rounded-lg transition-all duration-200 hover:scale-110 active:scale-95"
                          title="Download folder content"
                        >
                          <Download className="w-4 h-4 text-gray-500 dark:text-zinc-500 transition-transform duration-200" />
                        </button>
                      ) : (
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            handleDownload(file.id, file.name, file.size);
                          }}
                          className="p-2 hover:bg-gray-100 dark:hover:bg-zinc-800 rounded-lg transition-all duration-200 hover:scale-110 active:scale-95"
                        >
                          <Download className="w-4 h-4 text-gray-500 dark:text-zinc-500 transition-transform duration-200" />
                        </button>
                      )}
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          handleDelete(file.id);
                        }}
                        className="p-2 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-all duration-200 hover:scale-110 active:scale-95"
                      >
                        <Trash2 className="w-4 h-4 text-red-500 dark:text-red-400 transition-transform duration-200" />
                      </button>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

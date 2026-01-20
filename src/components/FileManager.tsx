import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/api/dialog';
import {
  Upload,
  FolderPlus,
  Download,
  Trash2,
  File,
  Folder,
  ChevronRight,
  Search,
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
  onDownloadTrigger?: (files: {id: string, name: string, size: number, destination: string}[]) => void;
}

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
  
  const [fileToDelete, setFileToDelete] = useState<FileMetadata | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);
  const [shouldAnimate, setShouldAnimate] = useState(true);
  const [folderToDownload, setFolderToDownload] = useState<FileMetadata | null>(null);
  const [folderStats, setFolderStats] = useState<FolderStats | null>(null);
  const [isGettingStats, setIsGettingStats] = useState(false);
  const refreshTimerRef = useRef<NodeJS.Timeout | null>(null);

  const loadFiles = async (showSkeleton = true, animate = true) => {
    setShouldAnimate(animate);
    if (showSkeleton) {
      setIsLoading(true);
    }
    
    try {
      const fileList = await invoke<FileMetadata[]>('list_files', {
        folder: currentFolder,
      });
      setFiles(fileList);
    } catch (error) {
      console.error('Failed to load files:', error);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadFiles(true, true);
    
    // Listen for file list refresh triggers (when an upload completes globally)
    let unlistenFn: (() => void) | null = null;
    
    const setupUploadListener = async () => {
      try {
        const unlisten = await listen('upload-progress', (event: any) => {
          const data = event.payload as any;
          if (data.status === 'completed' && data.folder === currentFolder) {
            // Debounced refresh: wait for other files to finish
            if (refreshTimerRef.current) clearTimeout(refreshTimerRef.current);
            refreshTimerRef.current = setTimeout(() => {
              loadFiles(false, false); // NO SKELETON, NO ANIMATION for background sync
              refreshTimerRef.current = null;
            }, 1000); 
          }
        });
        unlistenFn = unlisten;
      } catch (error) {
        console.error('Failed to setup refresh listener:', error);
      }
    };
    
    setupUploadListener();
    return () => { 
      if (unlistenFn) unlistenFn(); 
      if (refreshTimerRef.current) clearTimeout(refreshTimerRef.current);
    };
  }, [currentFolder]);

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

  const handleDelete = (fileId: string) => {
    const file = files.find(f => f.id === fileId);
    if (file) {
      setFileToDelete(file);
    }
  };

  const confirmDelete = async () => {
    if (!fileToDelete) return;
    
    setIsDeleting(true);
    try {
      if (fileToDelete.is_folder) {
        // Construct full path for folder deletion
        // If currentFolder is root "/", folder path is just "/" + name
        // Otherwise it's currentFolder + "/" + name
        const folderPath = currentFolder === '/' 
          ? `/${fileToDelete.name}`
          : `${currentFolder}/${fileToDelete.name}`;
          
        await invoke('delete_folder', { folderPath });
        toast?.showSuccess(`Folder "${fileToDelete.name}" deleted`, 2000);
      } else {
        await invoke('delete_file', { fileId: fileToDelete.id });
        toast?.showSuccess(`"${fileToDelete.name}" deleted`, 2000);
      }
      
      loadFiles(false);
    } catch (error) {
      console.error('Failed to delete item:', error);
      toast?.showError(`Failed to delete: ${error}`, 3000);
    } finally {
      setIsDeleting(false);
      setFileToDelete(null);
    }
  };

  const cancelDelete = () => {
    setFileToDelete(null);
  };

  const handleFolderDownloadRequest = async (folder: FileMetadata) => {
    setFolderToDownload(folder);
    setIsGettingStats(true);
    try {
      const folderPath = `${currentFolder}/${folder.name}`.replace('//', '/');
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
    const folderPath = `${currentFolder}/${folderName}`.replace('//', '/');
    
    setFolderToDownload(null);
    setFolderStats(null);

    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: `Select destination for "${folderName}"`
      });

      if (selected && !Array.isArray(selected)) {
        const allFiles = await invoke<FileMetadata[]>('list_files_recursive', { folderPath });
        
        if (allFiles.length === 0) {
          toast?.showWarning('Folder is empty');
          return;
        }

        const downloadItems = allFiles.map(file => ({
          id: file.id,
          name: file.name,
          size: file.size,
          destination: `${selected}/${file.name}`
        }));

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

  const handleFileClick = (file: FileMetadata) => {
    if (file.is_folder) {
      onFolderChange(`${currentFolder}/${file.name}`.replace('//', '/'));
    }
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

  const filteredFiles = files.filter((file) =>
    file.name.toLowerCase().includes(searchQuery.toLowerCase())
  );

  return (
    <div className="h-full flex flex-col">
      {showFolderDialog && (
        <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50 animate-fadeIn">
          <div className="bg-white rounded-2xl p-8 w-full max-w-md mx-4 shadow-large animate-scaleIn">
            <h2 className="text-lg font-semibold mb-6 text-gray-900">Create New Folder</h2>
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
        isOpen={!!fileToDelete}
        fileName={fileToDelete?.name || ''}
        onConfirm={confirmDelete}
        onCancel={cancelDelete}
        isDeleting={isDeleting}
      />

      {folderToDownload && (
        <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50 animate-fadeIn">
          <div className="bg-white rounded-2xl p-8 w-full max-w-md mx-4 shadow-large animate-scaleIn text-center">
            <div className="w-16 h-16 bg-gray-100 rounded-full flex items-center justify-center mx-auto mb-6">
              <Download className="w-8 h-8 text-gray-900" />
            </div>
            <h2 className="text-xl font-bold text-gray-900 mb-2">Download Folder?</h2>
            <p className="text-sm text-gray-500 mb-6 px-4">
              Are you sure you want to download <span className="font-bold text-gray-900">"{folderToDownload.name}"</span>?
            </p>
            
            <div className="bg-gray-50 rounded-2xl p-4 mb-8 flex justify-around">
              <div className="text-center">
                <p className="text-[10px] font-bold text-gray-400 uppercase tracking-widest mb-1">Items</p>
                <p className="text-lg font-bold text-gray-900">{isGettingStats ? '...' : folderStats?.file_count || 0}</p>
              </div>
              <div className="w-px h-10 bg-gray-200 my-auto" />
              <div className="text-center">
                <p className="text-[10px] font-bold text-gray-400 uppercase tracking-widest mb-1">Total Size</p>
                <p className="text-lg font-bold text-gray-900">{isGettingStats ? '...' : formatFileSize(folderStats?.total_size || 0)}</p>
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
      
      <div className="bg-white border-b border-gray-100 px-8 py-4">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center space-x-1.5 text-sm">
            <button
              onClick={() => onFolderChange('/')}
              className="text-gray-600 hover:text-gray-900 font-medium transition-colors"
            >
              Home
            </button>
            {currentFolder !== '/' &&
              currentFolder.split('/').filter(Boolean).map((part, index, arr) => (
                <div key={index} className="flex items-center space-x-1.5">
                  <ChevronRight className="w-3.5 h-3.5 text-gray-400" />
                  <button
                    onClick={() =>
                      onFolderChange('/' + arr.slice(0, index + 1).join('/'))
                    }
                    className="text-gray-600 hover:text-gray-900 font-medium transition-colors"
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
          <Search className="absolute left-3.5 top-1/2 transform -translate-y-1/2 w-4 h-4 text-gray-400 transition-colors duration-200 group-focus-within:text-gray-600" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search files..."
            className="input pl-10 transition-all duration-200 focus:shadow-soft focus:scale-[1.01]"
          />
        </div>
      </div>

      <div className="flex-1 overflow-auto px-8 py-6">
        {isLoading ? (
          <FileListSkeleton />
        ) : filteredFiles.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-gray-400 animate-fadeIn">
            <div className="w-16 h-16 bg-gray-100 rounded-2xl flex items-center justify-center mb-4 animate-bounce-subtle">
              <Folder className="w-8 h-8 text-gray-300" />
            </div>
            <p className="text-base font-semibold text-gray-600 mb-1">No files yet</p>
            <p className="text-sm text-gray-400">Upload your first file to get started</p>
          </div>
        ) : (
          <div className="space-y-1.5">
            {filteredFiles.map((file, index) => (
              <div
                key={file.id}
                className={`card-hover p-4 cursor-pointer group ${shouldAnimate ? 'animate-fadeIn' : ''}`}
                style={shouldAnimate ? { animationDelay: `${index * 0.03}s`, animationFillMode: 'both' } : {}}
                onClick={() => handleFileClick(file)}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center space-x-3.5 flex-1 min-w-0">
                    <div className={`flex-shrink-0 w-10 h-10 rounded-xl flex items-center justify-center transition-all duration-300 group-hover:scale-110 ${
                      file.is_folder ? 'bg-gray-900' : 'bg-gray-100'
                    }`}>
                      {file.is_folder ? (
                        <Folder className="w-5 h-5 text-white transition-transform duration-300 group-hover:scale-110" />
                      ) : (
                        <File className="w-5 h-5 text-gray-500 transition-transform duration-300 group-hover:scale-110" />
                      )}
                    </div>
                    <div className="flex-1 min-w-0">
                      <h3 className="font-medium text-gray-900 truncate text-sm">
                        {file.name}
                      </h3>
                      <p className="text-xs text-gray-400 mt-0.5">
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
                        className="p-2 hover:bg-gray-100 rounded-lg transition-all duration-200 hover:scale-110 active:scale-95"
                        title="Download folder content"
                      >
                        <Download className="w-4 h-4 text-gray-500 transition-transform duration-200" />
                      </button>
                    ) : (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          handleDownload(file.id, file.name, file.size);
                        }}
                        className="p-2 hover:bg-gray-100 rounded-lg transition-all duration-200 hover:scale-110 active:scale-95"
                      >
                        <Download className="w-4 h-4 text-gray-500 transition-transform duration-200" />
                      </button>
                    )}
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDelete(file.id);
                      }}
                      className="p-2 hover:bg-red-50 rounded-lg transition-all duration-200 hover:scale-110 active:scale-95"
                    >
                      <Trash2 className="w-4 h-4 text-red-500 transition-transform duration-200" />
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

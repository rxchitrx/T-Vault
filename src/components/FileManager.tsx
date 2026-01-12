import { useState, useEffect } from 'react';
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
import Confetti from './Confetti';
import { FileListSkeleton } from './SkeletonLoader';

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
}

export default function FileManager({
  currentFolder,
  onFolderChange,
  toast,
}: FileManagerProps) {
  const [files, setFiles] = useState<FileMetadata[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [showFolderDialog, setShowFolderDialog] = useState(false);
  const [newFolderName, setNewFolderName] = useState('');
  const [showConfetti, setShowConfetti] = useState(false);
  const [uploadProgress, setUploadProgress] = useState<{
    file: string;
    status: string;
    progress: number;
    error?: string;
  } | null>(null);

  useEffect(() => {
    loadFiles();
    
    // Listen for upload progress events
    let unlistenFn: (() => void) | null = null;
    
    const setupUploadListener = async () => {
      try {
        const unlisten = await listen('upload-progress', (event: any) => {
          const data = event.payload as any;
          setUploadProgress(data);
          
          // Clear progress after completion/error and refresh file list
          if (data.status === 'completed' || data.status === 'error') {
            if (data.status === 'completed') {
              setShowConfetti(true);
              toast?.showSuccess(`"${data.file}" uploaded successfully!`, 3000);
            } else {
              toast?.showError(data.error || 'Upload failed', 4000);
            }
            // Delay refresh slightly to ensure backend has finished processing
            setTimeout(() => {
              setUploadProgress(null);
              // Small delay before refresh to ensure metadata is saved
              setTimeout(() => {
                loadFiles();
              }, 500);
            }, 2000);
          }
        });
        
        unlistenFn = unlisten;
      } catch (error) {
        console.error('Failed to setup upload listener:', error);
      }
    };
    
    setupUploadListener();
    
    // Cleanup function
    return () => {
      if (unlistenFn) {
        unlistenFn();
      }
    };
  }, [currentFolder]);

  const loadFiles = async () => {
    setIsLoading(true);
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

  const handleUpload = async () => {
    try {
      const selected = await open({
        multiple: true,
        directory: false,
      });

      if (selected) {
        const filePaths = Array.isArray(selected) ? selected : [selected];

        for (const filePath of filePaths) {
          try {
            setUploadProgress({
              file: filePath.split('/').pop() || 'file',
              status: 'starting',
              progress: 0,
            });
            
            await invoke('upload_file', {
              filePath,
              folder: currentFolder,
            });
          } catch (error) {
            setUploadProgress({
              file: filePath.split('/').pop() || 'file',
              status: 'error',
              progress: 0,
              error: error as string,
            });
            setTimeout(() => setUploadProgress(null), 5000);
          }
        }
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
      console.log('Calling create_folder with:', { folderName: trimmedName, parentFolder: currentFolder });
      
      const result = await invoke('create_folder', {
        folderName: trimmedName,
        parentFolder: currentFolder,
      });
      
      console.log('Folder created successfully:', result);
      toast?.showSuccess(`Folder "${trimmedName}" created`, 2000);
      await loadFiles();
    } catch (error) {
      console.error('Failed to create folder:', error);
      toast?.showError(`Failed to create folder: ${error}`, 3000);
    }
  };

  const handleFolderDialogCancel = () => {
    setShowFolderDialog(false);
    setNewFolderName('');
  };

  const handleDelete = async (fileId: string) => {
    const file = files.find(f => f.id === fileId);
    if (confirm(`Are you sure you want to delete "${file?.name || 'this file'}"?`)) {
      try {
        await invoke('delete_file', { fileId });
        toast?.showSuccess(`"${file?.name || 'File'}" deleted`, 2000);
        loadFiles();
      } catch (error) {
        console.error('Failed to delete file:', error);
        toast?.showError('Failed to delete file', 3000);
      }
    }
  };

  const handleDownload = async (fileId: string, fileName: string) => {
    try {
      // Extract extension from filename for proper filter
      const ext = fileName.includes('.') ? fileName.split('.').pop() || '' : '';
      
      // Open save dialog
      const savePath = await save({
        defaultPath: fileName,
        filters: ext ? [{
          name: ext.toUpperCase() + ' Files',
          extensions: [ext]
        }] : undefined
      });

      if (savePath) {
        console.log('Download path:', savePath);
        toast?.showInfo(`Downloading "${fileName}"...`, 2000);

        await invoke('download_file', {
          fileId,
          destination: savePath,
        });

        toast?.showSuccess(`"${fileName}" downloaded successfully!`, 3000);
      }
    } catch (error) {
      console.error('Failed to download file:', error);
      toast?.showError(`Failed to download: ${error}`, 4000);
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
      {/* Confetti Effect */}
      {showConfetti && (
        <Confetti onComplete={() => setShowConfetti(false)} />
      )}

      {/* Folder Creation Dialog */}
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
      {/* Upload Progress */}
      {uploadProgress && (
        <div className={`px-8 py-3.5 border-b ${
          uploadProgress.status === 'error' 
            ? 'bg-red-50/50 border-red-100' 
            : uploadProgress.status === 'completed'
            ? 'bg-green-50/50 border-green-100'
            : 'bg-gray-50 border-gray-100'
        }`}>
          <div className="flex items-center justify-between">
            <div className="flex items-center space-x-3">
              <div className={`w-1.5 h-1.5 rounded-full ${
                uploadProgress.status === 'error' 
                  ? 'bg-red-500' 
                  : uploadProgress.status === 'completed'
                  ? 'bg-green-500'
                  : 'bg-gray-900 animate-pulse'
              }`} />
              <span className="text-sm font-medium text-gray-700">
                {uploadProgress.status === 'uploading' && 'Uploading...'}
                {uploadProgress.status === 'reading' && 'Reading file...'}
                {uploadProgress.status === 'completed' && 'Upload complete'}
                {uploadProgress.status === 'error' && 'Upload failed'}
                {uploadProgress.status === 'starting' && 'Starting upload...'}
              </span>
              <span className="text-sm text-gray-500">{uploadProgress.file}</span>
            </div>
            {uploadProgress.status !== 'completed' && uploadProgress.status !== 'error' && (
              <div className="flex items-center space-x-3">
                <div className="w-32 h-1 bg-gray-200 rounded-full overflow-hidden">
                  <div 
                    className="h-full bg-gray-900 transition-all duration-500 ease-out rounded-full"
                    style={{ width: `${uploadProgress.progress}%` }}
                  />
                </div>
                <span className="text-xs text-gray-500 font-medium animate-pulse">{uploadProgress.progress}%</span>
              </div>
            )}
            {uploadProgress.error && (
              <span className="text-xs text-red-600 font-medium">{uploadProgress.error}</span>
            )}
          </div>
        </div>
      )}

      {/* Toolbar */}
      <div className="bg-white border-b border-gray-100 px-8 py-4">
        <div className="flex items-center justify-between mb-4">
          {/* Breadcrumb */}
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

          {/* Actions */}
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
                console.log('New Folder button clicked');
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

        {/* Search */}
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

      {/* File List */}
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
                className="card-hover p-4 cursor-pointer group animate-fadeIn"
                style={{ animationDelay: `${index * 0.03}s`, animationFillMode: 'both' }}
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
                    {!file.is_folder && (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          handleDownload(file.id, file.name);
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

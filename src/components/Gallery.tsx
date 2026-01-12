import { useState, useEffect, useRef } from 'react';
import { invoke, convertFileSrc } from '@tauri-apps/api/tauri';
import { save } from '@tauri-apps/api/dialog';
import { Image as ImageIcon, Video, Download, Trash2, Grid3x3, List } from 'lucide-react';
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

type ViewMode = 'grid' | 'list';

interface ToastFunctions {
  showSuccess: (message: string, duration?: number) => void;
  showError: (message: string, duration?: number) => void;
  showInfo: (message: string, duration?: number) => void;
  showWarning: (message: string, duration?: number) => void;
}

interface GalleryProps {
  toast?: ToastFunctions;
}

export default function Gallery({ toast }: GalleryProps) {
  const [files, setFiles] = useState<FileMetadata[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [viewMode, setViewMode] = useState<ViewMode>('grid');
  const [selectedFile, setSelectedFile] = useState<FileMetadata | null>(null);
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [isLoadingPreview, setIsLoadingPreview] = useState(false);
  const [thumbnails, setThumbnails] = useState<Record<string, string>>({});
  const currentPreviewId = useRef<string | null>(null);
  
  const [fileToDelete, setFileToDelete] = useState<{id: string, name: string} | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);

  useEffect(() => {
    loadGalleryFiles();
  }, []);

  useEffect(() => {
    // Lazy load thumbnails for grid items with concurrency limit
    // Filter out files that already have thumbnails or are videos (backend skips videos anyway)
    const filesToLoad = files.filter(f => !thumbnails[f.id] && f.mime_type.startsWith('image/'));
    
    if (filesToLoad.length === 0) return;

    // Simple queue to load 3 at a time
    let active = 0;
    const concurrency = 3;
    let index = 0;
    let mounted = true;

    const loadNext = async () => {
        if (!mounted || index >= filesToLoad.length) return;

        while (active < concurrency && index < filesToLoad.length) {
            active++;
            const file = filesToLoad[index++];
            
            // Check cache again just in case
            if (thumbnails[file.id]) {
                active--;
                continue;
            }

            const thumbPath = `/tmp/tvault-thumb-${file.id}.jpg`;
            
            invoke<string | null>('download_thumbnail', {
                fileId: file.id,
                destination: thumbPath,
            }).then((result) => {
                if (!mounted) return;
                if (result) {
                    setThumbnails(prev => ({
                        ...prev,
                        [file.id]: convertFileSrc(result)
                    }));
                }
            }).catch(() => {
                // Ignore error
            }).finally(() => {
                if (!mounted) return;
                active--;
                loadNext();
            });
        }
    };

    loadNext();

    return () => {
        mounted = false;
    };
  }, [files]);

  const loadGalleryFiles = async () => {
    setIsLoading(true);
    try {
      // In a real implementation, this would fetch all image/video files
      const allFiles = await invoke<FileMetadata[]>('list_files', {
        folder: '/',
      });

      // Filter for images and videos
      const mediaFiles = allFiles.filter((file) =>
        file.mime_type.startsWith('image/') || file.mime_type.startsWith('video/')
      );

      setFiles(mediaFiles);
    } catch (error) {
      console.error('Failed to load gallery:', error);
      toast?.showError('Failed to load gallery files', 3000);
    } finally {
      setIsLoading(false);
    }
  };

  const isImage = (mimeType: string) => mimeType.startsWith('image/');

  const formatFileSize = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
  };

  const handleDownload = async (fileId: string, fileName: string) => {
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

  const handleDelete = (fileId: string, fileName: string) => {
    setFileToDelete({ id: fileId, name: fileName });
  };

  const confirmDelete = async () => {
    if (!fileToDelete) return;
    
    setIsDeleting(true);
    try {
      await invoke('delete_file', { fileId: fileToDelete.id });
      toast?.showSuccess(`"${fileToDelete.name}" deleted`, 2000);
      loadGalleryFiles();
    } catch (error) {
      console.error('Failed to delete file:', error);
      toast?.showError('Failed to delete file', 3000);
    } finally {
      setIsDeleting(false);
      setFileToDelete(null);
    }
  };

  const cancelDelete = () => {
    setFileToDelete(null);
  };

  const loadPreview = async (file: FileMetadata) => {
    // Update the current preview ID
    currentPreviewId.current = file.id;
    
    setIsLoadingPreview(true);
    setPreviewUrl(null);
    try {
      const isVideo = file.mime_type.startsWith('video/');
      
      // For videos, we do NOT want to download the full file automatically
      // because it could be huge (e.g. 650MB).
      // We'll try to get a thumbnail first.
      
      const thumbPath = `/tmp/tvault-thumb-${file.id}.jpg`;
      const thumbResult = await invoke<string | null>('download_thumbnail', {
        fileId: file.id,
        destination: thumbPath,
      });

      if (thumbResult && currentPreviewId.current === file.id) {
        setPreviewUrl(convertFileSrc(thumbResult));
        // If it's a video, we stop here. We show the thumbnail and let user click play.
        if (isVideo) {
           setIsLoadingPreview(false);
           return;
        }
      }

      // If we are here, we either don't have a thumbnail, or it's an image (so we want full res).
      // If it's a video and we missed the thumbnail, we still DO NOT auto-download full file.
      if (isVideo) {
        if (currentPreviewId.current === file.id) {
           setIsLoadingPreview(false);
           // Leave previewUrl as null, UI will show placeholder with "Preview not available"
           // We'll add a manual "Load Video" button in the UI
           return;
        }
      }

      // For images, we download the full file as it's usually reasonable size
      const tempPath = `/tmp/tvault-preview-${file.id}${file.name.substring(file.name.lastIndexOf('.'))}`;
      
      await invoke('download_file', {
        fileId: file.id,
        destination: tempPath,
      });

      // Check if this is still the current request
      if (currentPreviewId.current === file.id) {
        // Convert file path to a URL that Tauri can serve
        const assetUrl = convertFileSrc(tempPath);
        setPreviewUrl(assetUrl);
      }
    } catch (error) {
      if (currentPreviewId.current === file.id) {
        console.error('Failed to load preview:', error);
        // Don't show error toast for videos to avoid annoyance, just show placeholder
        if (!file.mime_type.startsWith('video/')) {
            toast?.showError('Failed to load preview', 3000);
        }
      }
    } finally {
      if (currentPreviewId.current === file.id) {
        setIsLoadingPreview(false);
      }
    }
  };

  // Function to manually trigger full video download
  const loadFullVideo = async (file: FileMetadata) => {
    setIsLoadingPreview(true);
    try {
       const tempPath = `/tmp/tvault-preview-${file.id}${file.name.substring(file.name.lastIndexOf('.'))}`;
       await invoke('download_file', {
        fileId: file.id,
        destination: tempPath,
      });
      if (currentPreviewId.current === file.id) {
        setPreviewUrl(convertFileSrc(tempPath));
      }
    } catch (error) {
       console.error(error);
       toast?.showError("Failed to load video", 3000);
    } finally {
       setIsLoadingPreview(false);
    }
  };

  useEffect(() => {
    if (selectedFile) {
      loadPreview(selectedFile);
    } else {
      // Clear current preview ID when modal is closed
      currentPreviewId.current = null;
      setPreviewUrl(null);
    }
  }, [selectedFile]);

  return (
    <div className="h-full flex flex-col">
      {/* Toolbar */}
      <div className="bg-white border-b border-gray-100 px-8 py-4">
        <div className="flex items-center justify-between">
          <h2 className="text-base font-semibold text-gray-900 tracking-tight">
            Gallery <span className="text-gray-400 font-normal">({files.length})</span>
          </h2>

          <div className="flex items-center space-x-1.5">
            <button
              onClick={() => setViewMode('grid')}
              className={`p-2 rounded-xl transition-all ${
                viewMode === 'grid'
                  ? 'bg-gray-900 text-white shadow-soft'
                  : 'bg-gray-50 text-gray-600 hover:bg-gray-100'
              }`}
            >
              <Grid3x3 className="w-4 h-4" />
            </button>
            <button
              onClick={() => setViewMode('list')}
              className={`p-2 rounded-xl transition-all ${
                viewMode === 'list'
                  ? 'bg-gray-900 text-white shadow-soft'
                  : 'bg-gray-50 text-gray-600 hover:bg-gray-100'
              }`}
            >
              <List className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>

      {/* Gallery Content */}
      <div className="flex-1 overflow-auto px-8 py-6">
        {isLoading ? (
          <div className="flex items-center justify-center h-full">
            <div className="flex flex-col items-center space-y-3 animate-fadeIn">
              <div className="w-8 h-8 border-2 border-gray-200 border-t-gray-900 rounded-full animate-spin"></div>
              <div className="text-sm text-gray-400 font-medium">Loading gallery...</div>
            </div>
          </div>
        ) : files.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-gray-400 animate-fadeIn">
            <div className="w-16 h-16 bg-gray-100 rounded-2xl flex items-center justify-center mb-4 animate-bounce-subtle">
              <ImageIcon className="w-8 h-8 text-gray-300" />
            </div>
            <p className="text-base font-semibold text-gray-600 mb-1">No media files</p>
            <p className="text-sm text-gray-400">Upload images or videos to see them here</p>
          </div>
        ) : viewMode === 'grid' ? (
          <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-3">
            {files.map((file, index) => (
              <div
                key={file.id}
                className="card-hover p-2 cursor-pointer group aspect-square animate-fadeIn"
                style={{ animationDelay: `${index * 0.05}s`, animationFillMode: 'both' }}
                onClick={() => setSelectedFile(file)}
              >
                <div className="relative w-full h-full bg-gray-100 rounded-xl overflow-hidden flex items-center justify-center">
                  {thumbnails[file.id] ? (
                      <img src={thumbnails[file.id]} className="w-full h-full object-cover" />
                  ) : isImage(file.mime_type) ? (
                    <ImageIcon className="w-10 h-10 text-gray-300" />
                  ) : (
                    <Video className="w-10 h-10 text-gray-300" />
                  )}

                  {/* Overlay with actions */}
                  <div className="absolute inset-0 bg-black/60 opacity-0 group-hover:opacity-100 transition-all duration-300 flex items-center justify-center space-x-2 backdrop-blur-sm">
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDownload(file.id, file.name);
                      }}
                      className="p-2 bg-white rounded-lg hover:bg-gray-50 transition-all duration-200 hover:scale-110 active:scale-95 transform"
                    >
                      <Download className="w-4 h-4 text-gray-700" />
                    </button>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDelete(file.id, file.name);
                      }}
                      className="p-2 bg-white rounded-lg hover:bg-red-50 transition-all duration-200 hover:scale-110 active:scale-95 transform"
                    >
                      <Trash2 className="w-4 h-4 text-red-500" />
                    </button>
                  </div>
                </div>

                <div className="mt-2 px-1">
                  <p className="text-xs font-medium text-gray-900 truncate">
                    {file.name}
                  </p>
                  <p className="text-xs text-gray-400 mt-0.5">{formatFileSize(file.size)}</p>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="space-y-1.5">
            {files.map((file, index) => (
              <div
                key={file.id}
                className="card-hover p-4 cursor-pointer group animate-fadeIn"
                style={{ animationDelay: `${index * 0.03}s`, animationFillMode: 'both' }}
                onClick={() => setSelectedFile(file)}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center space-x-3.5">
                    <div className="w-10 h-10 bg-gray-100 rounded-xl flex items-center justify-center overflow-hidden">
                      {thumbnails[file.id] ? (
                          <img src={thumbnails[file.id]} className="w-full h-full object-cover" />
                      ) : isImage(file.mime_type) ? (
                        <ImageIcon className="w-5 h-5 text-gray-500" />
                      ) : (
                        <Video className="w-5 h-5 text-gray-500" />
                      )}
                    </div>
                    <div>
                      <h3 className="font-medium text-gray-900 text-sm">{file.name}</h3>
                      <p className="text-xs text-gray-400 mt-0.5">
                        {formatFileSize(file.size)}
                      </p>
                    </div>
                  </div>

                  <div className="flex items-center space-x-1 opacity-0 group-hover:opacity-100 transition-opacity">
                    <button 
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDownload(file.id, file.name);
                      }}
                      className="p-2 hover:bg-gray-100 rounded-lg transition-colors"
                    >
                      <Download className="w-4 h-4 text-gray-500" />
                    </button>
                    <button 
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDelete(file.id, file.name);
                      }}
                      className="p-2 hover:bg-red-50 rounded-lg transition-colors"
                    >
                      <Trash2 className="w-4 h-4 text-red-500" />
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Preview Modal */}
      {selectedFile && (
        <div
          className="fixed inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center z-50 animate-fadeIn p-4"
          onClick={() => setSelectedFile(null)}
        >
          <div className="max-w-6xl w-full max-h-[90vh] bg-white rounded-2xl shadow-large animate-scaleIn overflow-hidden" onClick={(e) => e.stopPropagation()}>
            {/* Header */}
            <div className="flex items-center justify-between p-6 border-b border-gray-100">
              <h3 className="text-base font-semibold text-gray-900 truncate pr-4">{selectedFile.name}</h3>
              <div className="flex items-center space-x-2">
                <button
                  onClick={() => handleDownload(selectedFile.id, selectedFile.name)}
                  className="p-2 hover:bg-gray-100 rounded-lg transition-colors"
                  title="Download"
                >
                  <Download className="w-5 h-5 text-gray-600" />
                </button>
                <button
                  onClick={() => setSelectedFile(null)}
                  className="text-gray-400 hover:text-gray-600 transition-colors p-2"
                >
                  ✕
                </button>
              </div>
            </div>
            
            {/* Preview Content */}
            <div className="p-6 overflow-auto max-h-[calc(90vh-88px)]">
              <div className="bg-gray-50 rounded-xl min-h-[400px] flex items-center justify-center">
                {isLoadingPreview ? (
                  <div className="flex flex-col items-center space-y-3">
                    <div className="w-8 h-8 border-2 border-gray-200 border-t-gray-900 rounded-full animate-spin"></div>
                    <div className="text-sm text-gray-400 font-medium">Loading preview...</div>
                  </div>
                ) : previewUrl ? (
                  isImage(selectedFile.mime_type) ? (
                    <img 
                      src={previewUrl}
                      alt={selectedFile.name}
                      className="max-w-full max-h-[70vh] object-contain rounded-lg"
                    />
                  ) : (
                    <div className="relative w-full h-full flex flex-col items-center justify-center">
                        {/* If we have a URL for video, it might be a thumbnail if we implemented that, 
                            OR the full video. Since we differentiate by mime-type in loadPreview, 
                            if we are here with a video mime-type and a URL, it's likely the full video 
                            OR a thumbnail. 
                            Wait, loadPreview logic sets previewUrl to convertFileSrc(thumbResult).
                            If it was a thumbnail, we shouldn't use <video src=thumbnail>.
                            
                            We need a state to know if 'previewUrl' is a thumb or full video.
                            Let's rely on the file extension in the URL or a new state.
                            For simplicity: if it's a video file type, and we have a URL, check if it ends in .jpg/.png
                        */}
                       {previewUrl.toLowerCase().endsWith('.jpg') || previewUrl.toLowerCase().endsWith('.png') ? (
                           <div className="relative">
                               <img src={previewUrl} className="max-w-full max-h-[70vh] rounded-lg opacity-80" />
                               <div className="absolute inset-0 flex items-center justify-center">
                                   <button 
                                      onClick={() => loadFullVideo(selectedFile)}
                                      className="bg-gray-900/80 hover:bg-black text-white px-6 py-3 rounded-full flex items-center space-x-2 transition-transform hover:scale-105"
                                   >
                                       <Video className="w-6 h-6" />
                                       <span>Play Video</span>
                                   </button>
                               </div>
                           </div>
                       ) : (
                        <video 
                          src={previewUrl}
                          controls
                          className="max-w-full max-h-[70vh] rounded-lg"
                        >
                          Your browser does not support the video tag.
                        </video>
                       )}
                    </div>
                  )
                ) : (
                  <div className="flex flex-col items-center">
                    {isImage(selectedFile.mime_type) ? (
                      <ImageIcon className="w-20 h-20 text-gray-300 mb-4" />
                    ) : (
                      <Video className="w-20 h-20 text-gray-300 mb-4" />
                    )}
                    <p className="text-sm text-gray-400 mb-4">Preview not available</p>
                    {!isImage(selectedFile.mime_type) && (
                        <button 
                            onClick={() => loadFullVideo(selectedFile)}
                            className="btn btn-primary"
                        >
                            Load Full Video ({formatFileSize(selectedFile.size)})
                        </button>
                    )}
                  </div>
                )}
              </div>
            </div>
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
    </div>
  );
}

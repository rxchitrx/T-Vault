import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { Image as ImageIcon, Video, Download, Trash2, Grid3x3, List } from 'lucide-react';

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

  useEffect(() => {
    loadGalleryFiles();
  }, []);

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
                  {isImage(file.mime_type) ? (
                    <ImageIcon className="w-10 h-10 text-gray-300" />
                  ) : (
                    <Video className="w-10 h-10 text-gray-300" />
                  )}

                  {/* Overlay with actions */}
                  <div className="absolute inset-0 bg-black/60 opacity-0 group-hover:opacity-100 transition-all duration-300 flex items-center justify-center space-x-2 backdrop-blur-sm">
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        // Handle download
                      }}
                      className="p-2 bg-white rounded-lg hover:bg-gray-50 transition-all duration-200 hover:scale-110 active:scale-95 transform"
                    >
                      <Download className="w-4 h-4 text-gray-700" />
                    </button>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        // Handle delete
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
                    <div className="w-10 h-10 bg-gray-100 rounded-xl flex items-center justify-center">
                      {isImage(file.mime_type) ? (
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
                    <button className="p-2 hover:bg-gray-100 rounded-lg transition-colors">
                      <Download className="w-4 h-4 text-gray-500" />
                    </button>
                    <button className="p-2 hover:bg-red-50 rounded-lg transition-colors">
                      <Trash2 className="w-4 h-4 text-red-500" />
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Preview Modal (simplified) */}
      {selectedFile && (
        <div
          className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 animate-fadeIn"
          onClick={() => setSelectedFile(null)}
        >
          <div className="max-w-4xl max-h-[90vh] bg-white rounded-2xl p-6 shadow-large animate-scaleIn" onClick={(e) => e.stopPropagation()}>
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-base font-semibold text-gray-900">{selectedFile.name}</h3>
              <button
                onClick={() => setSelectedFile(null)}
                className="text-gray-400 hover:text-gray-600 transition-colors p-1"
              >
                ✕
              </button>
            </div>
            <div className="bg-gray-50 rounded-xl aspect-video flex items-center justify-center">
              {isImage(selectedFile.mime_type) ? (
                <ImageIcon className="w-20 h-20 text-gray-300" />
              ) : (
                <Video className="w-20 h-20 text-gray-300" />
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

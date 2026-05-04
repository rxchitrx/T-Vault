import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';
import { Download, Folder, X, HardDrive, FileIcon } from 'lucide-react';

interface FileDownloadInfo {
  file_id: string;
  file_name: string;
  file_size: number;
}

function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

export default function DownloadDialog() {
  const [isOpen, setIsOpen] = useState(false);
  const [files, setFiles] = useState<FileDownloadInfo[]>([]);
  const [saveMode, setSaveMode] = useState<'same' | 'individual'>('same');
  const [savePath, setSavePath] = useState('');
  const [individualPaths, setIndividualPaths] = useState<Record<string, string>>({});
  const [isDownloading, setIsDownloading] = useState(false);

  useEffect(() => {
    console.log('📥 [DLG] DownloadDialog: Setting up event listener for fuse-download-request');
    const unlisten = listen<FileDownloadInfo[]>('fuse-download-request', (event) => {
      const requestedFiles = event.payload;
      console.log('📥 [DLG] Received fuse-download-request event:', requestedFiles);
      console.log('📥 [DLG]   Files:', requestedFiles.map(f => `${f.file_name} (${formatSize(f.file_size)})`).join(', '));
      setFiles(requestedFiles);
      
      const defaultPath = `${process.env.HOME || '/Users'}/Downloads`;
      console.log('📥 [DLG] Default downloads path:', defaultPath);
      
      if (requestedFiles.length === 1) {
        const path = `${defaultPath}/${requestedFiles[0].file_name}`;
        console.log('📥 [DLG] Single file mode, setting save path:', path);
        setSavePath(path);
      } else {
        console.log('📥 [DLG] Multiple files mode (%d files), setting save mode to "same"', requestedFiles.length);
        setSavePath(defaultPath);
        setSaveMode('same');
      }
      
      const paths: Record<string, string> = {};
      requestedFiles.forEach(f => {
        paths[f.file_id] = `${defaultPath}/${f.file_name}`;
      });
      setIndividualPaths(paths);
      console.log('📥 [DLG] Individual paths set:', paths);
      
      setIsOpen(true);
      console.log('📥 [DLG] Dialog opened');
    });

    return () => {
      console.log('📥 [DLG] Cleaning up event listener');
      unlisten.then(fn => fn());
    };
  }, []);

  const handleBrowse = async (fileId?: string) => {
    console.log('📥 [DLG] handleBrowse called, fileId:', fileId || 'all');
    try {
      const file = fileId ? files.find(f => f.file_id === fileId) : files[0];
      const currentPath = saveMode === 'individual' && fileId ? individualPaths[fileId] : savePath;
      console.log('📥 [DLG] Opening native save dialog with path:', currentPath);
      
      const selected = await invoke<string>('select_save_location', {
        defaultPath: currentPath,
        fileName: file?.file_name || '',
      });
      
      console.log('📥 [DLG] User selected:', selected);
      
      if (fileId) {
        setIndividualPaths(prev => ({ ...prev, [fileId]: selected }));
        console.log('📥 [DLG] Updated individual path for', fileId, ':', selected);
      } else {
        setSavePath(selected);
        console.log('📥 [DLG] Updated save path:', selected);
      }
    } catch (e) {
      console.log('📥 [DLG] Browse cancelled or failed:', e);
    }
  };

  const handleDownload = async () => {
    console.log('📥 [DLG] handleDownload called');
    console.log('📥 [DLG]   Files:', files.map(f => f.file_name).join(', '));
    console.log('📥 [DLG]   Save mode:', saveMode);
    console.log('📥 [DLG]   Save path:', savePath);
    setIsDownloading(true);
    
    try {
      if (files.length === 1 || saveMode === 'same') {
        console.log('📥 [DLG] Sending SaveAllTo response, path:', savePath);
        await invoke('fuse_dialog_response', {
          result: { SaveAllTo: { path: savePath } },
        });
        console.log('📥 [DLG] SaveAllTo response sent successfully');
      } else {
        console.log('📥 [DLG] Sending individual SaveTo responses...');
        for (const file of files) {
          const path = individualPaths[file.file_id];
          if (path) {
            console.log('📥 [DLG]   Sending SaveTo for', file.file_name, '->', path);
            await invoke('fuse_dialog_response', {
              result: { SaveTo: { file_id: file.file_id, path } },
            });
          }
        }
        console.log('📥 [DLG] All SaveTo responses sent');
      }
    } catch (error) {
      console.error('📥 [DLG] Download response error:', error);
    }
    
    setIsDownloading(false);
    setIsOpen(false);
    console.log('📥 [DLG] Dialog closed');
  };

  const handleCancel = async () => {
    console.log('📥 [DLG] handleCancel called');
    try {
      if (files.length === 1) {
        console.log('📥 [DLG] Sending Cancel for single file:', files[0].file_id);
        await invoke('fuse_dialog_response', {
          result: { Cancel: { file_id: files[0].file_id } },
        });
      } else {
        console.log('📥 [DLG] Sending CancelAll for', files.length, 'files');
        await invoke('fuse_dialog_response', {
          result: 'CancelAll',
        });
      }
    } catch (error) {
      console.error('📥 [DLG] Cancel response error:', error);
    }
    
    setIsOpen(false);
    console.log('📥 [DLG] Dialog closed (cancelled)');
  };

  if (!isOpen) return null;

  const totalSize = files.reduce((sum, f) => sum + f.file_size, 0);
  const isMultiple = files.length > 1;

  return (
    <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50 animate-fadeIn">
      <div className="bg-white dark:bg-dark-surface rounded-2xl p-6 w-full max-w-md mx-4 shadow-large dark:shadow-large-dark animate-scaleIn">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center space-x-3">
            <div className="w-10 h-10 bg-blue-100 dark:bg-blue-900/30 rounded-xl flex items-center justify-center">
              <Download className="w-5 h-5 text-blue-600 dark:text-blue-400" />
            </div>
            <h3 className="text-lg font-semibold text-gray-900 dark:text-white">
              {isMultiple ? `Download ${files.length} Files` : 'Download File'}
            </h3>
          </div>
          <button onClick={handleCancel} className="p-1 rounded-lg hover:bg-gray-100 dark:hover:bg-zinc-800">
            <X className="w-4 h-4 text-gray-400" />
          </button>
        </div>

        <div className="bg-gray-50 dark:bg-zinc-900/50 rounded-xl p-3 mb-4 max-h-32 overflow-auto text-xs text-gray-600 dark:text-zinc-400 space-y-1">
          {files.map(file => (
            <div key={file.file_id} className="flex items-center justify-between">
              <div className="flex items-center space-x-2 truncate">
                <FileIcon className="w-3.5 h-3.5 shrink-0" />
                <span className="truncate font-medium text-gray-900 dark:text-white">{file.file_name}</span>
              </div>
              <span className="shrink-0 ml-2">{formatSize(file.file_size)}</span>
            </div>
          ))}
        </div>

        {isMultiple && (
          <div className="mb-4">
            <p className="text-xs text-gray-500 dark:text-zinc-500 mb-2">Total size: {formatSize(totalSize)}</p>
            <div className="space-y-2">
              <label className="flex items-center space-x-2 cursor-pointer">
                <input
                  type="radio"
                  name="saveMode"
                  checked={saveMode === 'same'}
                  onChange={() => setSaveMode('same')}
                  className="w-3.5 h-3.5 text-blue-600"
                />
                <span className="text-sm text-gray-700 dark:text-zinc-300">Save all to same location</span>
              </label>
              <label className="flex items-center space-x-2 cursor-pointer">
                <input
                  type="radio"
                  name="saveMode"
                  checked={saveMode === 'individual'}
                  onChange={() => setSaveMode('individual')}
                  className="w-3.5 h-3.5 text-blue-600"
                />
                <span className="text-sm text-gray-700 dark:text-zinc-300">Choose location for each file</span>
              </label>
            </div>
          </div>
        )}

        {saveMode === 'same' || !isMultiple ? (
          <div className="mb-4">
            <label className="text-xs text-gray-500 dark:text-zinc-500 mb-1 block">Save to</label>
            <div className="flex items-center space-x-2">
              <div className="flex-1 flex items-center space-x-2 bg-gray-100 dark:bg-zinc-800 rounded-lg px-3 py-2 text-sm text-gray-700 dark:text-zinc-300 truncate">
                <Folder className="w-4 h-4 shrink-0" />
                <span className="truncate">{savePath}</span>
              </div>
              <button
                onClick={() => handleBrowse()}
                className="btn btn-secondary text-xs px-3 py-2"
              >
                Browse
              </button>
            </div>
          </div>
        ) : (
          <div className="mb-4 space-y-2 max-h-40 overflow-auto">
            {files.map(file => (
              <div key={file.file_id} className="flex items-center space-x-2">
                <div className="flex-1 flex items-center space-x-2 bg-gray-100 dark:bg-zinc-800 rounded-lg px-3 py-2 text-xs text-gray-700 dark:text-zinc-300 truncate">
                  <FileIcon className="w-3.5 h-3.5 shrink-0" />
                  <span className="truncate">{individualPaths[file.file_id] || file.file_name}</span>
                </div>
                <button
                  onClick={() => handleBrowse(file.file_id)}
                  className="btn btn-secondary text-xs px-2 py-1.5"
                >
                  <HardDrive className="w-3.5 h-3.5" />
                </button>
              </div>
            ))}
          </div>
        )}

        <div className="flex justify-end space-x-3">
          <button
            onClick={handleCancel}
            disabled={isDownloading}
            className="btn btn-ghost flex-1"
          >
            Cancel
          </button>
          <button
            onClick={handleDownload}
            disabled={isDownloading || !savePath}
            className="btn btn-primary flex-1"
          >
            {isDownloading ? 'Downloading...' : 'Download'}
          </button>
        </div>
      </div>
    </div>
  );
}

import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';
import { ArrowUpCircle, LayoutGrid, Settings as SettingsIcon, FileText } from 'lucide-react';
import Sidebar from './Sidebar';
import FileManager from './FileManager';
import Gallery from './Gallery';
import Settings from './Settings';
import StorageStats from './StorageStats';
import TransferManager, { TransferItem } from './TransferManager';
import { ToastContainer, useToast } from './ToastContainer';

type View = 'files' | 'gallery' | 'settings';

export default function Dashboard() {
  const [currentView, setCurrentView] = useState<View>('files');
  const [currentFolder, setCurrentFolder] = useState('/');
  const [isTransferManagerOpen, setIsTransferManagerOpen] = useState(false);
  const [transferQueue, setTransferQueue] = useState<TransferItem[]>([]);
  const isProcessingQueue = useRef(false);
  const currentTransferId = useRef<string | null>(null);
  const processingTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  
  const toastHook = useToast();
  const { toasts, removeToast, showSuccess, showError, showInfo, showWarning } = toastHook;

  // Listen for upload progress events globally
  useEffect(() => {
    let unlistenUpload: (() => void) | null = null;
    let unlistenDownload: (() => void) | null = null;
    
    const setupListeners = async () => {
      try {
        unlistenUpload = await listen('upload-progress', (event: any) => {
          try {
            const data = event.payload as any;
            if (!data || !data.filePath) return;
            
            setTransferQueue(prev => {
              // If this item was cancelled (removed from queue), ignore progress
              if (!prev.find(item => item.id === data.filePath)) return prev;
              
              return prev.map(item => {
                if (item.type === 'upload' && item.id === data.filePath) {
                  if (data.status === 'completed') return { ...item, status: 'completed', progress: 100, currentBytes: item.totalBytes || data.total };
                  if (data.status === 'error') return { ...item, status: 'error', error: data.error || 'Upload failed' };
                  return { ...item, progress: data.progress || 0, currentBytes: data.current || 0, totalBytes: data.total || item.totalBytes || 0 };
                }
                return item;
              });
            });
          } catch (error) {
            console.error('Error handling upload progress:', error);
          }
        });

        unlistenDownload = await listen('download-progress', (event: any) => {
          try {
            const data = event.payload as any;
            if (!data || !data.fileId) return;
            
            setTransferQueue(prev => {
              // If this item was cancelled (removed from queue), ignore progress
              if (!prev.find(item => item.id === data.fileId)) return prev;

              return prev.map(item => {
                if (item.type === 'download' && item.id === data.fileId) {
                  if (data.status === 'completed') return { ...item, status: 'completed', progress: 100, currentBytes: item.totalBytes || data.total };
                  if (data.status === 'error') return { ...item, status: 'error', error: data.error || 'Download failed' };
                  return { ...item, progress: data.progress || 0, currentBytes: data.current || 0, totalBytes: data.total || item.totalBytes || 0 };
                }
                return item;
              });
            });
          } catch (error) {
            console.error('Error handling download progress:', error);
          }
        });
      } catch (error) {
        console.error('Failed to setup global listeners:', error);
      }
    };
    
    setupListeners();
    return () => { 
      if (unlistenUpload) unlistenUpload(); 
      if (unlistenDownload) unlistenDownload();
      if (processingTimeoutRef.current) clearTimeout(processingTimeoutRef.current);
    };
  }, []);

  // Queue Processor - triggers when queue changes
  useEffect(() => {
    // If we're already processing, check if the item we were processing still exists
    if (isProcessingQueue.current) {
      const activeItemExists = transferQueue.some(item => 
        item.id === currentTransferId.current && (item.status === 'uploading' || item.status === 'downloading')
      );
      
      // If active item was removed, we can allow a new process to start
      if (!activeItemExists && currentTransferId.current !== null) {
        console.log("Active item was cancelled/removed, allowing queue to advance");
        isProcessingQueue.current = false;
        currentTransferId.current = null;
      } else {
        return;
      }
    }
    
    const nextItem = transferQueue.find(item => item.status === 'pending');
    if (!nextItem) return;

    const processNext = async () => {
      isProcessingQueue.current = true;
      currentTransferId.current = nextItem.id;
      
      try {
        // Update status to active
        setTransferQueue(prev => prev.map(item => 
          item.id === nextItem.id ? { ...item, status: nextItem.type === 'upload' ? 'uploading' : 'downloading' } : item
        ));

        if (nextItem.type === 'upload') {
          await invoke('upload_file', {
            filePath: nextItem.id,
            folder: (nextItem as any).targetFolder || '/',
          });
        } else {
          await invoke('download_file', {
            fileId: nextItem.id,
            destination: (nextItem as any).destination,
          });
        }
      } catch (error) {
        // Only update error if item still exists in queue
        setTransferQueue(prev => {
          if (!prev.find(i => i.id === nextItem.id)) return prev;
          return prev.map(i => i.id === nextItem.id ? { ...i, status: 'error', error: String(error) } : i);
        });
      } finally {
        // Only reset if this was the item we were actually processing
        if (currentTransferId.current === nextItem.id) {
          isProcessingQueue.current = false;
          currentTransferId.current = null;
          
          setTimeout(() => {
            setTransferQueue(prev => [...prev]);
          }, 500);
        }
      }
    };

    processNext();
  }, [transferQueue]);

  const handleGlobalUpload = (filePaths: string[], targetFolder: string) => {
    const newItems: TransferItem[] = filePaths.map(path => ({
      id: path,
      name: path.split('/').pop() || 'file',
      size: 0,
      progress: 0,
      status: 'pending',
      type: 'upload',
      targetFolder 
    } as any));

    setTransferQueue(prev => [...prev, ...newItems]);
    showInfo(`Added ${filePaths.length} files to upload queue`, 2000);
  };

  const handleGlobalDownload = (files: {id: string, name: string, size: number, destination: string}[]) => {
    const newItems: TransferItem[] = files.map(file => ({
      id: file.id,
      name: file.name,
      size: file.size,
      progress: 0,
      status: 'pending',
      type: 'download',
      destination: file.destination
    } as any));

    setTransferQueue(prev => [...prev, ...newItems]);
    showInfo(`Added ${files.length} files to download queue`, 2000);
  };

  const removeTransferItem = (id: string) => {
    setTransferQueue(prev => prev.filter(item => item.id !== id));
  };

  const removeAllTransfers = () => {
    setTransferQueue([]);
    showWarning("All transfers canceled", 2000);
  };

  const clearCompletedTransfers = () => {
    setTransferQueue(prev => prev.filter(item => item.status === 'pending' || item.status === 'uploading' || item.status === 'downloading'));
  };

  const activeTransfersCount = transferQueue.filter(t => t.status === 'uploading' || t.status === 'downloading' || t.status === 'pending').length;

  return (
    <div className="h-screen w-screen flex bg-white font-sans text-gray-900 selection:bg-gray-900 selection:text-white">
      <ToastContainer toasts={toasts} onRemove={removeToast} />
      
      <TransferManager 
        queue={transferQueue}
        isOpen={isTransferManagerOpen}
        onClose={() => setIsTransferManagerOpen(false)}
        onClearCompleted={clearCompletedTransfers}
        onRemoveItem={removeTransferItem}
        onRemoveAll={removeAllTransfers}
      />

      <Sidebar currentView={currentView} onViewChange={setCurrentView} />

      <div className="flex-1 flex flex-col overflow-hidden">
        <div className="titlebar h-16 bg-white border-b border-gray-100 flex items-center justify-between px-8 z-10">
          <div className="flex items-center space-x-4">
            <div className="flex items-center space-x-2.5">
              {currentView === 'files' && <FileText className="w-5 h-5 text-gray-900" />}
              {currentView === 'gallery' && <LayoutGrid className="w-5 h-5 text-gray-900" />}
              {currentView === 'settings' && <SettingsIcon className="w-5 h-5 text-gray-900" />}
              <h2 className="text-lg font-bold tracking-tight">
                {currentView === 'files' && 'Files'}
                {currentView === 'gallery' && 'Gallery'}
                {currentView === 'settings' && 'Settings'}
              </h2>
            </div>
            
            {activeTransfersCount > 0 && (
              <button 
                onClick={() => setIsTransferManagerOpen(true)}
                className="flex items-center space-x-2 px-3 py-1 bg-gray-900 text-white rounded-full text-[10px] font-bold uppercase tracking-widest animate-pulse"
              >
                <ArrowUpCircle className="w-3 h-3" />
                <span>{activeTransfersCount} Processing</span>
              </button>
            )}
          </div>

          <div className="flex items-center space-x-4">
            <button
              onClick={() => setIsTransferManagerOpen(true)}
              className="p-2.5 hover:bg-gray-50 rounded-xl transition-all duration-200 group relative"
            >
              <ArrowUpCircle className={`w-5 h-5 transition-colors ${isTransferManagerOpen ? 'text-gray-900' : 'text-gray-400 group-hover:text-gray-600'}`} />
              {activeTransfersCount > 0 && (
                <span className="absolute top-2 right-2 w-2 h-2 bg-gray-900 rounded-full border-2 border-white" />
              )}
            </button>
            <div className="w-px h-6 bg-gray-100" />
            <StorageStats />
          </div>
        </div>

        <div className="flex-1 overflow-hidden bg-[#FAFAFA]">
          {currentView === 'files' && (
            <div className="h-full">
              <FileManager
                currentFolder={currentFolder}
                onFolderChange={setCurrentFolder}
                toast={{ showSuccess, showError, showInfo, showWarning }}
                onUploadTrigger={handleGlobalUpload}
                onDownloadTrigger={handleGlobalDownload}
              />
            </div>
          )}
          {currentView === 'gallery' && (
            <div className="h-full">
              <Gallery toast={{ showSuccess, showError, showInfo, showWarning }} />
            </div>
          )}
          {currentView === 'settings' && (
            <div className="h-full">
              <Settings />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}


import { X, Upload, CheckCircle2, Clock, AlertCircle, Trash2, Download } from 'lucide-react';

export interface TransferItem {
  id: string;
  name: string;
  size: number;
  progress: number;
  currentBytes?: number;
  totalBytes?: number;
  status: 'pending' | 'uploading' | 'downloading' | 'completed' | 'error';
  type: 'upload' | 'download';
  error?: string;
}

interface TransferManagerProps {
  queue: TransferItem[];
  isOpen: boolean;
  onClose: () => void;
  onClearCompleted: () => void;
  onRemoveItem: (id: string) => void;
  onRemoveAll: () => void;
}

export default function TransferManager({
  queue,
  isOpen,
  onClose,
  onClearCompleted,
  onRemoveItem,
  onRemoveAll,
}: TransferManagerProps) {
  if (!isOpen) return null;

  const activeTransfer = queue.find((t) => t.status === 'uploading' || t.status === 'downloading');
  const pendingTransfers = queue.filter((t) => t.status === 'pending');
  const finishedTransfers = queue.filter(
    (t) => t.status === 'completed' || t.status === 'error'
  );

  const formatFileSize = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
  };

  return (
    <div className="fixed inset-0 bg-black/40 backdrop-blur-sm z-[60] flex justify-end animate-fadeIn">
      <div
        className="w-full max-w-md bg-white dark:bg-dark-surface h-full shadow-2xl dark:shadow-large-dark flex flex-col animate-slideInRight"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="px-6 py-5 border-b border-gray-100 dark:border-dark-border flex items-center justify-between">
          <div>
            <h2 className="text-lg font-bold text-gray-900 dark:text-white">Transfers</h2>
            <p className="text-xs text-gray-500 dark:text-zinc-500 mt-0.5">
              {queue.length} items in total
            </p>
          </div>
          <div className="flex items-center space-x-2">
            {queue.length > 0 && (
              <button
                onClick={onRemoveAll}
                className="p-2 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-full transition-colors group"
                title="Cancel all transfers"
              >
                <Trash2 className="w-5 h-5 text-gray-400 dark:text-zinc-600 group-hover:text-red-500 dark:group-hover:text-red-400" />
              </button>
            )}
            <button
              onClick={onClose}
              className="p-2 hover:bg-gray-100 dark:hover:bg-zinc-800 rounded-full transition-colors"
            >
              <X className="w-5 h-5 text-gray-500 dark:text-zinc-600" />
            </button>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto">
          {/* Active Transfer */}
          {activeTransfer && (
            <div className="p-6 bg-gray-50 dark:bg-zinc-900/30 border-b border-gray-100 dark:border-dark-border">
              <div className="flex items-center justify-between mb-4">
                <h3 className="text-xs font-bold text-gray-400 dark:text-zinc-600 uppercase tracking-wider">
                  {activeTransfer.type === 'upload' ? 'Currently Uploading' : 'Currently Downloading'}
                </h3>
                <button
                  onClick={() => onRemoveItem(activeTransfer.id)}
                  className="text-[10px] font-bold text-red-500 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 uppercase tracking-wider"
                >
                  Cancel
                </button>
              </div>
              <div className="bg-white dark:bg-zinc-800 p-4 rounded-2xl shadow-sm border border-gray-100 dark:border-zinc-700">
                <div className="flex items-center space-x-3 mb-3">
                  <div className="p-2 bg-gray-900 dark:bg-white rounded-lg">
                    {activeTransfer.type === 'upload' ? (
                      <Upload className="w-4 h-4 text-white dark:text-gray-900 animate-bounce-subtle" />
                    ) : (
                      <Download className="w-4 h-4 text-white dark:text-gray-900 animate-bounce-subtle" />
                    )}
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-bold text-gray-900 dark:text-white truncate">
                      {activeTransfer.name}
                    </p>
                    <p className="text-xs text-gray-500 dark:text-zinc-500">
                      {activeTransfer.currentBytes !== undefined && activeTransfer.totalBytes !== undefined ? (
                        `${formatFileSize(activeTransfer.currentBytes)} of ${formatFileSize(activeTransfer.totalBytes)}`
                      ) : (
                        activeTransfer.size > 0 ? formatFileSize(activeTransfer.size) : 'Processing...'
                      )}
                    </p>
                  </div>
                  <span className="text-sm font-bold text-gray-900 dark:text-white">
                    {activeTransfer.progress}%
                  </span>
                </div>
                <div className="w-full h-2 bg-gray-100 dark:bg-zinc-700 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-gray-900 dark:bg-white transition-all duration-300 ease-out"
                    style={{ width: `${activeTransfer.progress}%` }}
                  />
                </div>
              </div>
            </div>
          )}

          {/* Pending Queue */}
          <div className="p-6">
            {pendingTransfers.length > 0 && (
              <div className="mb-8">
                <h3 className="text-xs font-bold text-gray-400 dark:text-zinc-600 uppercase tracking-wider mb-4 flex items-center">
                  <Clock className="w-3.5 h-3.5 mr-1.5" />
                  Queue ({pendingTransfers.length})
                </h3>
                <div className="space-y-3">
                  {pendingTransfers.map((item) => (
                    <div key={item.id} className="flex items-center space-x-3 group">
                      <div className="w-8 h-8 rounded-lg bg-gray-50 dark:bg-zinc-800 flex items-center justify-center border border-gray-100 dark:border-zinc-700">
                        {item.type === 'upload' ? (
                          <Upload className="w-3 h-3 text-gray-400 dark:text-zinc-600" />
                        ) : (
                          <Download className="w-3 h-3 text-gray-400 dark:text-zinc-600" />
                        )}
                      </div>
                      <div className="flex-1 min-w-0">
                        <p className="text-sm font-medium text-gray-700 dark:text-gray-300 truncate">
                          {item.name}
                        </p>
                        <p className="text-[10px] text-gray-400 dark:text-zinc-600">
                          {item.size > 0 ? formatFileSize(item.size) : 'Pending...'} • {item.type}
                        </p>
                      </div>
                      <button
                        onClick={() => onRemoveItem(item.id)}
                        className="p-1.5 hover:bg-gray-100 dark:hover:bg-zinc-800 rounded-md opacity-0 group-hover:opacity-100 transition-all"
                        title="Remove from queue"
                      >
                        <X className="w-3.5 h-3.5 text-gray-400 dark:text-zinc-600 hover:text-red-500 dark:hover:text-red-400" />
                      </button>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Completed/Errors */}
            {finishedTransfers.length > 0 && (
              <div>
                <div className="flex items-center justify-between mb-4">
                  <h3 className="text-xs font-bold text-gray-400 dark:text-zinc-600 uppercase tracking-wider">
                    Recent
                  </h3>
                  <button
                    onClick={onClearCompleted}
                    className="text-[10px] font-bold text-gray-400 dark:text-zinc-600 hover:text-gray-900 dark:hover:text-white uppercase tracking-wider"
                  >
                    Clear All
                  </button>
                </div>
                <div className="space-y-4">
                  {finishedTransfers.map((item) => (
                    <div key={item.id} className="flex items-center space-x-3 group">
                      <div className={`w-8 h-8 rounded-lg flex items-center justify-center ${
                        item.status === 'error' ? 'bg-red-50 dark:bg-red-900/20' : 'bg-green-50 dark:bg-green-900/20'
                      }`}>
                        {item.status === 'error' ? (
                          <AlertCircle className="w-4 h-4 text-red-500 dark:text-red-400" />
                        ) : (
                          <CheckCircle2 className="w-4 h-4 text-green-500 dark:text-green-400" />
                        )}
                      </div>
                      <div className="flex-1 min-w-0">
                        <p className="text-sm font-medium text-gray-700 dark:text-gray-300 truncate">
                          {item.name}
                        </p>
                        {item.status === 'error' ? (
                          <p className="text-[10px] text-red-500 dark:text-red-400 truncate font-medium">
                            {item.error || 'Failed'}
                          </p>
                        ) : (
                          <p className="text-[10px] text-green-600 dark:text-green-400 font-medium uppercase tracking-tight">
                            {item.type} Completed
                          </p>
                        )}
                      </div>
                      <button
                        onClick={() => onRemoveItem(item.id)}
                        className="p-1.5 hover:bg-gray-100 dark:hover:bg-zinc-800 rounded-md opacity-0 group-hover:opacity-100 transition-all"
                        title="Remove from list"
                      >
                        <X className="w-3.5 h-3.5 text-gray-400 dark:text-zinc-600 hover:text-gray-600 dark:hover:text-zinc-500" />
                      </button>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {queue.length === 0 && (
              <div className="flex flex-col items-center justify-center py-20 text-center">
                <div className="w-16 h-16 bg-gray-50 dark:bg-zinc-800 rounded-2xl flex items-center justify-center mb-4">
                  <Clock className="w-8 h-8 text-gray-200 dark:text-zinc-700" />
                </div>
                <p className="text-sm font-bold text-gray-900 dark:text-white">No active transfers</p>
                <p className="text-xs text-gray-400 dark:text-zinc-600 mt-1 px-10">
                  Files you upload or download will appear here
                </p>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

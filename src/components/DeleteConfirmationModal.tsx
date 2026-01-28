import { AlertTriangle } from 'lucide-react';

interface DeleteConfirmationModalProps {
  isOpen: boolean;
  itemNames: string[];
  onConfirm: () => void;
  onCancel: () => void;
  isDeleting?: boolean;
}

export default function DeleteConfirmationModal({
  isOpen,
  itemNames,
  onConfirm,
  onCancel,
  isDeleting = false,
}: DeleteConfirmationModalProps) {
  if (!isOpen) return null;

  const itemCount = itemNames.length;
  const title = itemCount > 1 ? `Delete ${itemCount} items?` : 'Delete item?';
  const description = itemCount > 1
    ? `You're about to delete ${itemCount} items. This action cannot be undone.`
    : 'Are you sure you want to delete this item? This action cannot be undone.';

  return (
    <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50 animate-fadeIn">
      <div className="bg-white dark:bg-dark-surface rounded-2xl p-6 w-full max-w-sm mx-4 shadow-large dark:shadow-large-dark animate-scaleIn">
        <div className="flex items-center justify-center w-12 h-12 bg-red-100 dark:bg-red-900/30 rounded-full mb-4 mx-auto">
          <AlertTriangle className="w-6 h-6 text-red-600 dark:text-red-400" />
        </div>

        <h3 className="text-lg font-semibold text-center text-gray-900 dark:text-white mb-2">
          {title}
        </h3>

        <p className="text-sm text-center text-gray-500 dark:text-zinc-500 mb-4">
          {description}
        </p>

        <div className="bg-gray-50 dark:bg-zinc-900/50 rounded-xl p-3 mb-6 max-h-40 overflow-auto text-xs text-gray-600 dark:text-zinc-400">
          {itemNames.map((name, index) => (
            <p key={`${name}-${index}`} className="truncate">
              • <span className="font-medium text-gray-900 dark:text-white">{name}</span>
            </p>
          ))}
        </div>

        <div className="flex justify-end space-x-3">
          <button
            onClick={onCancel}
            disabled={isDeleting}
            className="btn btn-ghost flex-1"
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            disabled={isDeleting}
            className="btn bg-red-600 hover:bg-red-700 dark:bg-red-700 dark:hover:bg-red-600 text-white flex-1 focus:ring-red-200 dark:focus:ring-red-900/30"
          >
            {isDeleting ? 'Deleting...' : 'Delete'}
          </button>
        </div>
      </div>
    </div>
  );
}

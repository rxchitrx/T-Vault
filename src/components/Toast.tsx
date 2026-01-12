import { useEffect } from 'react';
import { CheckCircle2, XCircle, Info, AlertCircle, X } from 'lucide-react';

export interface Toast {
  id: string;
  message: string;
  type: 'success' | 'error' | 'info' | 'warning';
  duration?: number;
}

interface ToastProps {
  toast: Toast;
  onRemove: (id: string) => void;
}

export default function ToastComponent({ toast, onRemove }: ToastProps) {
  useEffect(() => {
    const timer = setTimeout(() => {
      onRemove(toast.id);
    }, toast.duration || 3000);

    return () => clearTimeout(timer);
  }, [toast.id, toast.duration, onRemove]);

  const icons = {
    success: <CheckCircle2 className="w-5 h-5 text-green-600" />,
    error: <XCircle className="w-5 h-5 text-red-600" />,
    info: <Info className="w-5 h-5 text-blue-600" />,
    warning: <AlertCircle className="w-5 h-5 text-yellow-600" />,
  };

  const bgColors = {
    success: 'bg-green-50 border-green-200',
    error: 'bg-red-50 border-red-200',
    info: 'bg-blue-50 border-blue-200',
    warning: 'bg-yellow-50 border-yellow-200',
  };

  return (
    <div
      className={`${bgColors[toast.type]} border rounded-xl p-4 shadow-large flex items-center space-x-3 animate-slideUp min-w-[300px] max-w-md`}
      style={{ animationDelay: '0s' }}
    >
      <div className="flex-shrink-0">{icons[toast.type]}</div>
      <p className="flex-1 text-sm font-medium text-gray-900">{toast.message}</p>
      <button
        onClick={() => onRemove(toast.id)}
        className="flex-shrink-0 text-gray-400 hover:text-gray-600 transition-colors p-1 rounded-lg hover:bg-white/50"
      >
        <X className="w-4 h-4" />
      </button>
    </div>
  );
}

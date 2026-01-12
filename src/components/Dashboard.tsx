import { useState } from 'react';
import Sidebar from './Sidebar';
import FileManager from './FileManager';
import Gallery from './Gallery';
import Settings from './Settings';
import StorageStats from './StorageStats';
import { ToastContainer, useToast } from './ToastContainer';


type View = 'files' | 'gallery' | 'settings';

export default function Dashboard() {
  const [currentView, setCurrentView] = useState<View>('files');
  const [currentFolder, setCurrentFolder] = useState('/');
  const toastHook = useToast();
  const { toasts, removeToast, showSuccess, showError, showInfo, showWarning } = toastHook;

  return (
    <div className="h-screen w-screen flex bg-white">
      {/* Toast Container */}
      <ToastContainer 
        toasts={toasts} 
        onRemove={removeToast}
      />

      {/* Sidebar */}
      <Sidebar
        currentView={currentView}
        onViewChange={setCurrentView}
      />

      {/* Main Content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Titlebar (macOS drag region) */}
        <div className="titlebar h-14 bg-white border-b border-gray-100 flex items-center justify-between px-8">
          <div className="flex items-center space-x-3">
            <h2 className="text-base font-semibold text-gray-900 tracking-tight">
              {currentView === 'files' && 'Files'}
              {currentView === 'gallery' && 'Gallery'}
              {currentView === 'settings' && 'Settings'}
            </h2>
          </div>
          <StorageStats />
        </div>

        {/* Content Area */}
        <div className="flex-1 overflow-hidden bg-gray-50">
          {currentView === 'files' && (
            <div className="h-full animate-fadeIn">
              <FileManager
                currentFolder={currentFolder}
                onFolderChange={setCurrentFolder}
                toast={{ showSuccess, showError, showInfo, showWarning }}
              />
            </div>
          )}
          {currentView === 'gallery' && (
            <div className="h-full animate-fadeIn">
              <Gallery toast={{ showSuccess, showError, showInfo, showWarning }} />
            </div>
          )}
          {currentView === 'settings' && (
            <div className="h-full animate-fadeIn">
              <Settings />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

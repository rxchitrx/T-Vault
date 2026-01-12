import { Cloud, FolderOpen, Image, Settings as SettingsIcon } from 'lucide-react';

interface SidebarProps {
  currentView: string;
  onViewChange: (view: 'files' | 'gallery' | 'settings') => void;
}

export default function Sidebar({ currentView, onViewChange }: SidebarProps) {
  const menuItems = [
    { id: 'files', icon: FolderOpen, label: 'Files' },
    { id: 'gallery', icon: Image, label: 'Gallery' },
    { id: 'settings', icon: SettingsIcon, label: 'Settings' },
  ];

  return (
    <div className="w-64 bg-white border-r border-gray-100 flex flex-col">
      {/* Logo */}
      <div className="titlebar h-14 px-6 flex items-center border-b border-gray-100">
        <div className="flex items-center space-x-2.5">
          <div className="w-7 h-7 bg-gray-900 rounded-lg flex items-center justify-center">
            <Cloud className="w-4 h-4 text-white" />
          </div>
          <span className="font-semibold text-base text-gray-900 tracking-tight">T-Vault</span>
        </div>
      </div>

      {/* Menu */}
      <nav className="flex-1 px-3 py-6 space-y-1">
        {menuItems.map((item) => {
          const Icon = item.icon;
          const isActive = currentView === item.id;

          return (
            <button
              key={item.id}
              onClick={() => onViewChange(item.id as any)}
              className={`w-full flex items-center space-x-3 px-3.5 py-2.5 rounded-xl transition-all duration-300 relative overflow-hidden group ${
                isActive
                  ? 'bg-gray-900 text-white shadow-soft transform scale-[1.02]'
                  : 'text-gray-600 hover:bg-gray-50 hover:text-gray-900 hover:translate-x-1'
              }`}
            >
              {isActive && (
                <div className="absolute inset-0 shimmer-effect" />
              )}
              <Icon className={`w-4 h-4 transition-transform duration-300 relative z-10 ${isActive ? 'text-white scale-110' : 'text-gray-500 group-hover:scale-110'}`} />
              <span className={`text-sm font-medium transition-all duration-300 relative z-10 ${isActive ? 'text-white' : 'text-gray-700'}`}>
                {item.label}
              </span>
            </button>
          );
        })}
      </nav>

      {/* Footer */}
      <div className="p-5 border-t border-gray-100">
        <div className="text-xs text-gray-400 text-center space-y-0.5">
          <p className="font-medium">Powered by Telegram</p>
          <p>Unlimited Storage</p>
        </div>
      </div>
    </div>
  );
}

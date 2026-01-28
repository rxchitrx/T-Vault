import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';
import {
  User,
  Bell,
  Shield,
  Info,
  LogOut,
  ExternalLink,
  RefreshCcw,
  Database,
  Moon,
  Sun,
} from 'lucide-react';
import { useToast } from './ToastContainer';
import { useTheme } from '../contexts/ThemeContext';

export default function Settings() {
  const { isDark, toggleTheme } = useTheme();
  const [notifications, setNotifications] = useState(true);
  const [autoSync, setAutoSync] = useState(true);
  const [isSyncing, setIsSyncing] = useState(false);
  const [isMigrating, setIsMigrating] = useState(false);
  const [migrationProgress, setMigrationProgress] = useState<{current: number, total: number, file: string, progress: number} | null>(null);
  const toast = useToast();

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen('migration-progress', (event: any) => {
        setMigrationProgress(event.payload);
    }).then(u => unlisten = u);
    
    return () => { if(unlisten) unlisten(); };
  }, []);

  const handleMigration = async () => {
    if (!confirm("This will move existing files from 'Saved Messages' to their respective folder channels. This process involves downloading and re-uploading each file. Continue?")) return;
    
    setIsMigrating(true);
    toast.showInfo('Starting migration...', 3000);
    
    try {
      const report = await invoke<any>('migrate_files_to_folders');
      toast.showSuccess(`Migration complete! Moved: ${report.migrated}, Failed: ${report.failed}, Skipped: ${report.skipped}`, 5000);
    } catch (error) {
      console.error('Migration failed:', error);
      toast.showError(`Migration failed: ${error}`, 4000);
    } finally {
      setIsMigrating(false);
      setMigrationProgress(null);
    }
  };

  const handleSync = async () => {
    setIsSyncing(true);
    toast.showInfo('Scanning Telegram for your files...', 3000);
    
    try {
      const count = await invoke<number>('sync_metadata');
      toast.showSuccess(`Sync complete! Found ${count} files.`, 4000);
    } catch (error) {
      console.error('Sync failed:', error);
      toast.showError(`Sync failed: ${error}`, 4000);
    } finally {
      setIsSyncing(false);
    }
  };

  return (
    <div className="h-full overflow-auto px-8 py-6">
      <div className="max-w-3xl mx-auto space-y-5 pb-10">
        {/* Account Section */}
        <div className="card p-6">
          <div className="flex items-center space-x-3 mb-6">
            <div className="w-8 h-8 bg-gray-900 dark:bg-white rounded-xl flex items-center justify-center">
              <User className="w-4 h-4 text-white dark:text-gray-900" />
            </div>
            <h2 className="text-base font-semibold text-gray-900 dark:text-white tracking-tight">Account</h2>
          </div>

          <div className="space-y-4">
            <div className="flex items-center justify-between py-3 border-b border-gray-100 dark:border-dark-border">
              <div>
                <p className="font-medium text-gray-900 dark:text-white text-sm">Connected Account</p>
                <p className="text-xs text-gray-400 dark:text-zinc-500 mt-0.5">Telegram</p>
              </div>
              <button className="btn btn-secondary text-sm">
                <LogOut className="w-4 h-4 mr-2" />
                Sign Out
              </button>
            </div>

            <div className="py-3">
              <p className="font-medium text-gray-900 dark:text-white mb-2 text-sm">Storage Backend</p>
              <p className="text-xs text-gray-500 dark:text-zinc-500 leading-relaxed">
                Your files are stored in your Telegram "Saved Messages". Only you can
                access them.
              </p>
            </div>
          </div>
        </div>

        {/* Maintenance & Sync Section */}
        <div className="card p-6">
          <div className="flex items-center space-x-3 mb-6">
            <div className="w-8 h-8 bg-gray-900 dark:bg-white rounded-xl flex items-center justify-center">
              <Database className="w-4 h-4 text-white dark:text-gray-900" />
            </div>
            <h2 className="text-base font-semibold text-gray-900 dark:text-white tracking-tight">Maintenance & Sync</h2>
          </div>

          <div className="space-y-4">
            <div className="flex items-center justify-between py-3">
              <div className="flex-1 pr-10">
                <p className="font-medium text-gray-900 dark:text-white text-sm">Rebuild Library</p>
                <p className="text-xs text-gray-400 dark:text-zinc-500 mt-1.5 leading-relaxed">
                  Lost your file list after an update or reinstall? This will scan your Telegram Saved Messages and restore all files uploaded with this app.
                </p>
              </div>
              <button
                onClick={handleSync}
                disabled={isSyncing || isMigrating}
                className={`btn btn-primary text-sm whitespace-nowrap ${isSyncing ? 'opacity-50 cursor-not-allowed' : ''}`}
              >
                <RefreshCcw className={`w-4 h-4 mr-2 ${isSyncing ? 'animate-spin' : ''}`} />
                {isSyncing ? 'Syncing...' : 'Sync Now'}
              </button>
            </div>

            <div className="flex items-center justify-between py-3 border-t border-gray-100 dark:border-dark-border">
              <div className="flex-1 pr-10">
                <p className="font-medium text-gray-900 dark:text-white text-sm">Migrate to Folder Channels</p>
                <p className="text-xs text-gray-400 dark:text-zinc-500 mt-1.5 leading-relaxed">
                  Move existing files from "Saved Messages" to dedicated folder channels. This improves organization and enables folder-specific features.
                </p>
                {isMigrating && migrationProgress && (
                  <div className="mt-3">
                    <div className="flex justify-between text-xs text-gray-500 dark:text-zinc-500 mb-1">
                      <span>Moving: {migrationProgress.file}</span>
                      <span>{migrationProgress.current}/{migrationProgress.total}</span>
                    </div>
                    <div className="w-full bg-gray-100 dark:bg-zinc-800 rounded-full h-1.5">
                      <div
                        className="bg-gray-900 dark:bg-white h-1.5 rounded-full transition-all duration-300"
                        style={{ width: `${migrationProgress.progress}%` }}
                      ></div>
                    </div>
                  </div>
                )}
              </div>
              <button
                onClick={handleMigration}
                disabled={isMigrating || isSyncing}
                className={`btn btn-secondary text-sm whitespace-nowrap ${isMigrating ? 'opacity-50 cursor-not-allowed' : ''}`}
              >
                {isMigrating ? 'Migrating...' : 'Start Migration'}
              </button>
            </div>
          </div>
        </div>

        {/* Security Section */}

        <div className="card p-6">
          <div className="flex items-center space-x-3 mb-6">
            <div className="w-8 h-8 bg-gray-900 dark:bg-white rounded-xl flex items-center justify-center">
              <Shield className="w-4 h-4 text-white dark:text-gray-900" />
            </div>
            <h2 className="text-base font-semibold text-gray-900 dark:text-white tracking-tight">Security</h2>
          </div>

          <div className="space-y-4">
            <div className="flex items-center justify-between py-3 border-b border-gray-100 dark:border-dark-border">
              <div className="flex-1">
                <p className="font-medium text-gray-900 dark:text-white text-sm">End-to-End Encryption</p>
                <p className="text-xs text-gray-400 dark:text-zinc-500 mt-0.5">
                  All files are automatically encrypted before uploading
                </p>
              </div>
              <div className="flex items-center space-x-2">
                <Shield className="w-4 h-4 text-green-600 dark:text-green-500" />
                <span className="text-xs font-medium text-green-600 dark:text-green-500">Always On</span>
              </div>
            </div>

            <div className="py-3">
              <div className="p-3 bg-gray-50 dark:bg-zinc-900/50 border border-gray-100 dark:border-zinc-800 rounded-xl">
                <p className="text-xs text-gray-600 dark:text-zinc-400 leading-relaxed">
                  <strong className="font-semibold text-gray-900 dark:text-white">Direct Access:</strong> Files are uploaded directly to Telegram Saved Messages, allowing you to view and access them from any Telegram client (phone, desktop, web).
                </p>
              </div>
            </div>
          </div>
        </div>

        {/* Preferences Section */}
        <div className="card p-6">
          <div className="flex items-center space-x-3 mb-6">
            <div className="w-8 h-8 bg-gray-900 dark:bg-white rounded-xl flex items-center justify-center">
              <Bell className="w-4 h-4 text-white dark:text-gray-900" />
            </div>
            <h2 className="text-base font-semibold text-gray-900 dark:text-white tracking-tight">Preferences</h2>
          </div>

          <div className="space-y-4">
            <div className="flex items-center justify-between py-3 border-b border-gray-100 dark:border-dark-border">
              <div className="flex-1">
                <p className="font-medium text-gray-900 dark:text-white text-sm">Dark Mode</p>
                <p className="text-xs text-gray-400 dark:text-zinc-500 mt-0.5">
                  Switch between light and dark themes
                </p>
              </div>
              <button
                onClick={toggleTheme}
                className="relative inline-flex items-center justify-center w-10 h-10 rounded-xl bg-gray-100 dark:bg-zinc-800 hover:bg-gray-200 dark:hover:bg-zinc-700 transition-all duration-200"
              >
                {isDark ? (
                  <Sun className="w-5 h-5 text-gray-700 dark:text-yellow-400" />
                ) : (
                  <Moon className="w-5 h-5 text-gray-700" />
                )}
              </button>
            </div>

            <div className="flex items-center justify-between py-3 border-b border-gray-100 dark:border-dark-border">
              <div className="flex-1">
                <p className="font-medium text-gray-900 dark:text-white text-sm">Notifications</p>
                <p className="text-xs text-gray-400 dark:text-zinc-500 mt-0.5">
                  Get notified about upload/download progress
                </p>
              </div>
              <label className="relative inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  checked={notifications}
                  onChange={(e) => setNotifications(e.target.checked)}
                  className="sr-only peer"
                />
                <div className="w-10 h-5 bg-gray-200 dark:bg-zinc-700 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-gray-300 dark:peer-focus:ring-zinc-600 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white dark:peer-checked:after:border-gray-900 after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 dark:after:border-zinc-600 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-gray-900 dark:peer-checked:bg-white"></div>
              </label>
            </div>

            <div className="flex items-center justify-between py-3">
              <div className="flex-1">
                <p className="font-medium text-gray-900 dark:text-white text-sm">Auto Sync</p>
                <p className="text-xs text-gray-400 dark:text-zinc-500 mt-0.5">
                  Automatically sync changes with Telegram
                </p>
              </div>
              <label className="relative inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  checked={autoSync}
                  onChange={(e) => setAutoSync(e.target.checked)}
                  className="sr-only peer"
                />
                <div className="w-10 h-5 bg-gray-200 dark:bg-zinc-700 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-gray-300 dark:peer-focus:ring-zinc-600 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white dark:peer-checked:after:border-gray-900 after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 dark:after:border-zinc-600 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-gray-900 dark:peer-checked:bg-white"></div>
              </label>
            </div>
          </div>
        </div>

        {/* About Section */}
        <div className="card p-6">
          <div className="flex items-center space-x-3 mb-6">
            <div className="w-8 h-8 bg-gray-900 dark:bg-white rounded-xl flex items-center justify-center">
              <Info className="w-4 h-4 text-white dark:text-gray-900" />
            </div>
            <h2 className="text-base font-semibold text-gray-900 dark:text-white tracking-tight">About</h2>
          </div>

          <div className="space-y-4">
            <div className="py-3 border-b border-gray-100 dark:border-dark-border">
              <p className="text-xs text-gray-500 dark:text-zinc-500 mb-2 font-medium">Version 1.0.1</p>
              <p className="text-xs text-gray-500 dark:text-zinc-500 leading-relaxed">
                T-Vault provides unlimited cloud storage by leveraging Telegram as
                storage backend.
              </p>
            </div>

            <div className="flex items-center space-x-4">
              <a
                href="https://github.com/inulute/t-vault"
                target="_blank"
                rel="noopener noreferrer"
                className="btn btn-secondary"
              >
                <ExternalLink className="w-4 h-4 mr-2" />
                GitHub
              </a>
              <a
                href="https://telegram.org"
                target="_blank"
                rel="noopener noreferrer"
                className="btn btn-secondary"
              >
                <ExternalLink className="w-4 h-4 mr-2" />
                Telegram
              </a>
            </div>

            <div className="pt-4 border-t border-gray-100 dark:border-dark-border">
              <p className="text-xs text-gray-400 dark:text-zinc-600 leading-relaxed">
                ⚠️ Important: This application uses Telegram for file storage. Please
                ensure you comply with Telegram's Terms of Service. Use responsibly and
                avoid excessive automated uploads that could result in account
                restrictions.
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

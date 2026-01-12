import { useState } from 'react';
import {
  User,
  Bell,
  Shield,
  Info,
  LogOut,
  ExternalLink,
} from 'lucide-react';

export default function Settings() {
  const [notifications, setNotifications] = useState(true);
  const [autoSync, setAutoSync] = useState(true);

  return (
    <div className="h-full overflow-auto px-8 py-6">
      <div className="max-w-3xl mx-auto space-y-5">
        {/* Account Section */}
        <div className="card p-6">
          <div className="flex items-center space-x-3 mb-6">
            <div className="w-8 h-8 bg-gray-900 rounded-xl flex items-center justify-center">
              <User className="w-4 h-4 text-white" />
            </div>
            <h2 className="text-base font-semibold text-gray-900 tracking-tight">Account</h2>
          </div>

          <div className="space-y-4">
            <div className="flex items-center justify-between py-3 border-b border-gray-100">
              <div>
                <p className="font-medium text-gray-900 text-sm">Connected Account</p>
                <p className="text-xs text-gray-400 mt-0.5">Telegram</p>
              </div>
              <button className="btn btn-secondary text-sm">
                <LogOut className="w-4 h-4 mr-2" />
                Sign Out
              </button>
            </div>

            <div className="py-3">
              <p className="font-medium text-gray-900 mb-2 text-sm">Storage Backend</p>
              <p className="text-xs text-gray-500 leading-relaxed">
                Your files are stored in your Telegram "Saved Messages". Only you can
                access them.
              </p>
            </div>
          </div>
        </div>

        {/* Security Section */}
        <div className="card p-6">
          <div className="flex items-center space-x-3 mb-6">
            <div className="w-8 h-8 bg-gray-900 rounded-xl flex items-center justify-center">
              <Shield className="w-4 h-4 text-white" />
            </div>
            <h2 className="text-base font-semibold text-gray-900 tracking-tight">Security</h2>
          </div>

          <div className="space-y-4">
            <div className="flex items-center justify-between py-3 border-b border-gray-100">
              <div className="flex-1">
                <p className="font-medium text-gray-900 text-sm">End-to-End Encryption</p>
                <p className="text-xs text-gray-400 mt-0.5">
                  All files are automatically encrypted before uploading
                </p>
              </div>
              <div className="flex items-center space-x-2">
                <Shield className="w-4 h-4 text-green-600" />
                <span className="text-xs font-medium text-green-600">Always On</span>
              </div>
            </div>

            <div className="py-3">
              <div className="p-3 bg-gray-50 border border-gray-100 rounded-xl">
                <p className="text-xs text-gray-600 leading-relaxed">
                  <strong className="font-semibold text-gray-900">Direct Access:</strong> Files are uploaded directly to Telegram Saved Messages, allowing you to view and access them from any Telegram client (phone, desktop, web).
                </p>
              </div>
            </div>
          </div>
        </div>

        {/* Preferences Section */}
        <div className="card p-6">
          <div className="flex items-center space-x-3 mb-6">
            <div className="w-8 h-8 bg-gray-900 rounded-xl flex items-center justify-center">
              <Bell className="w-4 h-4 text-white" />
            </div>
            <h2 className="text-base font-semibold text-gray-900 tracking-tight">Preferences</h2>
          </div>

          <div className="space-y-4">
            <div className="flex items-center justify-between py-3 border-b border-gray-100">
              <div className="flex-1">
                <p className="font-medium text-gray-900 text-sm">Notifications</p>
                <p className="text-xs text-gray-400 mt-0.5">
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
                <div className="w-10 h-5 bg-gray-200 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-gray-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-gray-900"></div>
              </label>
            </div>

            <div className="flex items-center justify-between py-3">
              <div className="flex-1">
                <p className="font-medium text-gray-900 text-sm">Auto Sync</p>
                <p className="text-xs text-gray-400 mt-0.5">
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
                <div className="w-10 h-5 bg-gray-200 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-gray-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-gray-900"></div>
              </label>
            </div>
          </div>
        </div>

        {/* About Section */}
        <div className="card p-6">
          <div className="flex items-center space-x-3 mb-6">
            <div className="w-8 h-8 bg-gray-900 rounded-xl flex items-center justify-center">
              <Info className="w-4 h-4 text-white" />
            </div>
            <h2 className="text-base font-semibold text-gray-900 tracking-tight">About</h2>
          </div>

          <div className="space-y-4">
            <div className="py-3 border-b border-gray-100">
              <p className="text-xs text-gray-500 mb-2 font-medium">Version 1.0.0</p>
              <p className="text-xs text-gray-500 leading-relaxed">
                UnlimCloud provides unlimited cloud storage by leveraging Telegram as the
                storage backend.
              </p>
            </div>

            <div className="flex items-center space-x-4">
              <a
                href="https://github.com/inulute/unlim-cloud"
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

            <div className="pt-4 border-t border-gray-100">
              <p className="text-xs text-gray-400 leading-relaxed">
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

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { Cloud, KeyRound, AlertCircle, ExternalLink } from 'lucide-react';

interface ApiKeyScreenProps {
  onKeysSaved: () => void;
}

export default function ApiKeyScreen({ onKeysSaved }: ApiKeyScreenProps) {
  const [apiId, setApiId] = useState('');
  const [apiHash, setApiHash] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState('');

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    
    // Validate inputs
    if (!apiId.trim()) {
      setError('API ID is required');
      return;
    }
    
    if (!apiHash.trim()) {
      setError('API Hash is required');
      return;
    }

    const apiIdNum = parseInt(apiId.trim(), 10);
    if (isNaN(apiIdNum) || apiIdNum <= 0) {
      setError('API ID must be a valid positive number');
      return;
    }

    setIsLoading(true);

    try {
      await invoke('save_api_keys', {
        apiId: apiIdNum,
        apiHash: apiHash.trim(),
      });
      
      // Keys saved successfully, proceed to login
      onKeysSaved();
    } catch (err) {
      setError(err as string);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="h-screen w-screen flex items-center justify-center bg-gray-50">
      <div className="w-full max-w-md mx-4">
        <div className="card p-10 animate-fadeIn">
          {/* Logo */}
          <div className="text-center mb-10 animate-fadeIn">
            <div className="inline-flex items-center justify-center w-14 h-14 bg-gray-900 rounded-2xl mb-5 animate-scaleIn">
              <Cloud className="w-7 h-7 text-white animate-bounce-subtle" />
            </div>
            <h1 className="text-3xl font-semibold text-gray-900 mb-2 tracking-tight animate-slideUp" style={{ animationDelay: '0.1s', animationFillMode: 'both' }}>
              Welcome to UnlimCloud
            </h1>
            <p className="text-sm text-gray-500 animate-slideUp" style={{ animationDelay: '0.2s', animationFillMode: 'both' }}>
              First, let's set up your Telegram API credentials
            </p>
          </div>

          {/* Instructions */}
          <div className="mb-6 p-4 bg-blue-50 border border-blue-100 rounded-xl animate-fadeIn" style={{ animationDelay: '0.3s', animationFillMode: 'both' }}>
            <div className="flex items-start space-x-3">
              <ExternalLink className="w-5 h-5 text-blue-600 flex-shrink-0 mt-0.5" />
              <div className="flex-1">
                <p className="text-sm font-medium text-blue-900 mb-1">Get Your API Credentials</p>
                <ol className="text-xs text-blue-700 space-y-1 list-decimal list-inside">
                  <li>Visit <a href="https://my.telegram.org" target="_blank" rel="noopener noreferrer" className="underline font-medium">my.telegram.org</a></li>
                  <li>Log in with your phone number</li>
                  <li>Click "API development tools"</li>
                  <li>Create a new application</li>
                  <li>Copy your <code className="bg-blue-100 px-1 rounded">api_id</code> and <code className="bg-blue-100 px-1 rounded">api_hash</code></li>
                </ol>
              </div>
            </div>
          </div>

          {/* Error Message */}
          {error && (
            <div className="mb-6 p-4 bg-red-50 border border-red-100 rounded-xl flex items-start space-x-3 animate-fadeIn">
              <AlertCircle className="w-5 h-5 text-red-600 flex-shrink-0 mt-0.5" />
              <p className="text-sm text-red-700 flex-1">{error}</p>
            </div>
          )}

          {/* Form */}
          <form onSubmit={handleSubmit} className="space-y-5 animate-fadeIn" style={{ animationDelay: '0.4s', animationFillMode: 'both' }}>
            {/* API ID */}
            <div>
              <label htmlFor="apiId" className="block text-sm font-medium text-gray-700 mb-2">
                API ID
              </label>
              <div className="relative">
                <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                  <KeyRound className="h-5 w-5 text-gray-400" />
                </div>
                <input
                  id="apiId"
                  type="text"
                  inputMode="numeric"
                  value={apiId}
                  onChange={(e) => setApiId(e.target.value)}
                  placeholder="12345678"
                  className="input pl-10 w-full"
                  disabled={isLoading}
                  autoFocus
                />
              </div>
            </div>

            {/* API Hash */}
            <div>
              <label htmlFor="apiHash" className="block text-sm font-medium text-gray-700 mb-2">
                API Hash
              </label>
              <div className="relative">
                <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                  <KeyRound className="h-5 w-5 text-gray-400" />
                </div>
                <input
                  id="apiHash"
                  type="text"
                  value={apiHash}
                  onChange={(e) => setApiHash(e.target.value)}
                  placeholder="abcdef1234567890abcdef1234567890"
                  className="input pl-10 w-full font-mono text-sm"
                  disabled={isLoading}
                />
              </div>
            </div>

            {/* Submit Button */}
            <button
              type="submit"
              disabled={isLoading}
              className="btn-primary w-full py-3 text-base font-medium disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {isLoading ? (
                <span className="flex items-center justify-center">
                  <svg className="animate-spin -ml-1 mr-3 h-5 w-5 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                    <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
                    <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                  </svg>
                  Saving...
                </span>
              ) : (
                'Continue'
              )}
            </button>
          </form>

          {/* Footer Note */}
          <p className="mt-6 text-xs text-center text-gray-500 animate-fadeIn" style={{ animationDelay: '0.5s', animationFillMode: 'both' }}>
            Your credentials are stored securely on your device and never shared.
          </p>
        </div>
      </div>
    </div>
  );
}

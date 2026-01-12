import { Cloud } from 'lucide-react';

export default function LoadingScreen() {
  return (
    <div className="h-screen w-screen flex items-center justify-center bg-gradient-to-br from-telegram-primary to-telegram-dark">
      <div className="text-center">
        <div className="mb-8 flex justify-center">
          <Cloud className="w-24 h-24 text-white animate-pulse" />
        </div>
        <h1 className="text-4xl font-bold text-white mb-2">T-Vault</h1>
        <p className="text-white/80 text-lg">Unlimited Cloud Storage</p>
        <div className="mt-8 flex justify-center space-x-2">
          <div className="w-3 h-3 bg-white rounded-full animate-bounce" style={{ animationDelay: '0ms' }}></div>
          <div className="w-3 h-3 bg-white rounded-full animate-bounce" style={{ animationDelay: '150ms' }}></div>
          <div className="w-3 h-3 bg-white rounded-full animate-bounce" style={{ animationDelay: '300ms' }}></div>
        </div>
      </div>
    </div>
  );
}

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { Cloud, Phone, KeyRound, AlertCircle } from 'lucide-react';

interface LoginScreenProps {
  onLoginSuccess: () => void;
}

export default function LoginScreen({ onLoginSuccess }: LoginScreenProps) {
  const [phone, setPhone] = useState('');
  const [code, setCode] = useState('');
  const [step, setStep] = useState<'phone' | 'code'>('phone');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState('');

  // Check if already authenticated on mount
  useEffect(() => {
    const checkAuth = async () => {
      try {
        const isAuth = await invoke<boolean>('initialize_client');
        if (isAuth) {
          onLoginSuccess();
        }
      } catch (err) {
        // Not authenticated, continue with login flow
        console.log('Not authenticated, showing login screen');
      }
    };
    checkAuth();
  }, [onLoginSuccess]);

  const handleSendCode = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setIsLoading(true);

    try {
      await invoke('telegram_login', { phone });
      setStep('code');
    } catch (err) {
      setError(err as string);
    } finally {
      setIsLoading(false);
    }
  };

  const handleVerifyCode = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setIsLoading(true);

    try {
      await invoke('telegram_verify_code', { phone, code });
      
      onLoginSuccess();
    } catch (err) {
      setError(err as string);
      setIsLoading(false);
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
            <h1 className="text-3xl font-semibold text-gray-900 mb-2 tracking-tight animate-slideUp" style={{ animationDelay: '0.1s', animationFillMode: 'both' }}>UnlimCloud</h1>
            <p className="text-sm text-gray-500 animate-slideUp" style={{ animationDelay: '0.2s', animationFillMode: 'both' }}>Sign in with your Telegram account</p>
          </div>

          {/* Error Message */}
          {error && (
            <div className="mb-6 p-4 bg-red-50 border border-red-100 rounded-xl flex items-start space-x-3 animate-fadeIn">
              <AlertCircle className="w-5 h-5 text-red-600 flex-shrink-0 mt-0.5" />
              <div className="text-sm text-red-700">{error}</div>
            </div>
          )}

          {/* Phone Step */}
          {step === 'phone' && (
            <form onSubmit={handleSendCode} className="space-y-5">
              <div>
                <label className="block text-xs font-medium text-gray-700 mb-2.5 uppercase tracking-wide">
                  Phone Number
                </label>
                <div className="relative">
                  <Phone className="absolute left-4 top-1/2 transform -translate-y-1/2 w-4 h-4 text-gray-400" />
                  <input
                    type="tel"
                    value={phone}
                    onChange={(e) => setPhone(e.target.value)}
                    placeholder="+1234567890"
                    className="input pl-11"
                    required
                    disabled={isLoading}
                  />
                </div>
                <p className="mt-2 text-xs text-gray-400">
                  Enter your phone number with country code
                </p>
              </div>

              <button
                type="submit"
                className="btn btn-primary w-full"
                disabled={isLoading}
              >
                {isLoading ? 'Sending...' : 'Continue'}
              </button>
            </form>
          )}

          {/* Code Step */}
          {step === 'code' && (
            <form onSubmit={handleVerifyCode} className="space-y-5">
              <div>
                <label className="block text-xs font-medium text-gray-700 mb-2.5 uppercase tracking-wide">
                  Verification Code
                </label>
                <div className="relative">
                  <KeyRound className="absolute left-4 top-1/2 transform -translate-y-1/2 w-4 h-4 text-gray-400" />
                  <input
                    type="text"
                    value={code}
                    onChange={(e) => setCode(e.target.value)}
                    placeholder="12345"
                    className="input pl-11 text-center text-2xl tracking-[0.5em] font-medium"
                    required
                    disabled={isLoading}
                    autoFocus
                  />
                </div>
                <p className="mt-2 text-xs text-gray-400">
                  Enter the code sent to {phone}
                </p>
              </div>

              <div className="space-y-2.5">
                <button
                  type="submit"
                  className="btn btn-primary w-full"
                  disabled={isLoading}
                >
                  {isLoading ? 'Verifying...' : 'Verify Code'}
                </button>
                <button
                  type="button"
                  onClick={() => setStep('phone')}
                  className="btn btn-ghost w-full"
                  disabled={isLoading}
                >
                  Change Number
                </button>
              </div>
            </form>
          )}

          {/* Info */}
          <div className="mt-8 pt-6 border-t border-gray-100">
            <p className="text-xs text-gray-400 text-center leading-relaxed">
              By signing in, you agree to use Telegram as your storage backend.
              Your files will be stored in your Telegram account.
            </p>
          </div>
        </div>

        {/* Note */}
        <div className="mt-6 text-center">
          <p className="text-xs text-gray-500">
            Don't have a Telegram account?{' '}
            <a
              href="https://telegram.org"
              target="_blank"
              rel="noopener noreferrer"
              className="font-medium text-gray-700 hover:text-gray-900 underline underline-offset-2"
            >
              Create one here
            </a>
          </p>
        </div>
      </div>
    </div>
  );
}

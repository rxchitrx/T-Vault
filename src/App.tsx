import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import ApiKeyScreen from './components/ApiKeyScreen';
import LoginScreen from './components/LoginScreen';
import Dashboard from './components/Dashboard';
import LoadingScreen from './components/LoadingScreen';

function App() {
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [apiKeysConfigured, setApiKeysConfigured] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    initialize();
  }, []);

  const initialize = async () => {
    try {
      // First check if API keys are configured
      const keysConfigured = await invoke<boolean>('check_api_keys_configured');
      setApiKeysConfigured(keysConfigured);
      
      // If keys are configured, check authentication
      if (keysConfigured) {
        try {
          const authenticated = await invoke<boolean>('telegram_check_auth');
          setIsAuthenticated(authenticated);
        } catch (error) {
          console.error('Auth check failed:', error);
          setIsAuthenticated(false);
        }
      }
    } catch (error) {
      console.error('Initialization failed:', error);
      setApiKeysConfigured(false);
    } finally {
      setIsLoading(false);
    }
  };

  const handleKeysSaved = () => {
    setApiKeysConfigured(true);
    // After keys are saved, user will proceed to login screen
  };

  const handleLoginSuccess = () => {
    setIsAuthenticated(true);
  };

  if (isLoading) {
    return <LoadingScreen />;
  }

  return (
    <div className="h-screen w-screen overflow-hidden">
      {isAuthenticated ? (
        <Dashboard />
      ) : apiKeysConfigured ? (
        <LoginScreen onLoginSuccess={handleLoginSuccess} />
      ) : (
        <ApiKeyScreen onKeysSaved={handleKeysSaved} />
      )}
    </div>
  );
}

export default App;

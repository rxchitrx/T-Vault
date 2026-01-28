import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { HardDrive } from 'lucide-react';

interface StorageStats {
  total_files: number;
  total_size: number;
  folder_count: number;
}

export default function StorageStats() {
  const [stats, setStats] = useState<StorageStats | null>(null);

  useEffect(() => {
    loadStats();
    const interval = setInterval(loadStats, 30000); // Update every 30 seconds
    return () => clearInterval(interval);
  }, []);

  const loadStats = async () => {
    try {
      const data = await invoke<StorageStats>('get_storage_stats');
      setStats(data);
    } catch (error) {
      console.error('Failed to load stats:', error);
    }
  };

  const formatSize = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
  };

  if (!stats) return null;

  return (
    <div className="flex items-center space-x-4 text-xs">
      <div className="flex items-center space-x-2 text-gray-500 dark:text-zinc-500">
        <HardDrive className="w-3.5 h-3.5" />
        <span className="font-medium">{formatSize(stats.total_size)} used</span>
      </div>
      <div className="w-1 h-1 rounded-full bg-gray-300 dark:bg-zinc-600" />
      <div className="text-gray-500 dark:text-zinc-500 font-medium">{stats.total_files} files</div>
    </div>
  );
}

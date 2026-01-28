export function FileSkeleton() {
  return (
    <div className="card p-4 animate-fadeIn">
      <div className="flex items-center justify-between">
        <div className="flex items-center space-x-3.5 flex-1 min-w-0">
          <div className="flex-shrink-0 w-10 h-10 rounded-xl bg-gray-100 dark:bg-zinc-800 animate-skeleton" />
          <div className="flex-1 min-w-0 space-y-2">
            <div className="h-4 bg-gray-100 dark:bg-zinc-800 rounded-lg animate-skeleton w-3/4" />
            <div className="h-3 bg-gray-100 dark:bg-zinc-800 rounded-lg animate-skeleton w-1/2" />
          </div>
        </div>
      </div>
    </div>
  );
}

export function FileListSkeleton() {
  return (
    <div className="space-y-1.5">
      {Array.from({ length: 5 }).map((_, i) => (
        <FileSkeleton key={i} />
      ))}
    </div>
  );
}

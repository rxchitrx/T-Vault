import { useEffect, useState } from 'react';

interface ConfettiProps {
  onComplete?: () => void;
}

export default function Confetti({ onComplete }: ConfettiProps) {
  const [particles, setParticles] = useState<Array<{
    id: number;
    left: number;
    delay: number;
    duration: number;
    color: string;
  }>>([]);

  useEffect(() => {
    const colors = ['#000000', '#374151', '#6B7280', '#9CA3AF'];
    const newParticles = Array.from({ length: 50 }, (_, i) => ({
      id: i,
      left: Math.random() * 100,
      delay: Math.random() * 0.5,
      duration: 1 + Math.random() * 0.5,
      color: colors[Math.floor(Math.random() * colors.length)],
    }));

    setParticles(newParticles);

    const timer = setTimeout(() => {
      if (onComplete) onComplete();
    }, 2000);

    return () => clearTimeout(timer);
  }, [onComplete]);

  return (
    <div className="fixed inset-0 pointer-events-none z-50 overflow-hidden">
      {particles.map((particle) => (
        <div
          key={particle.id}
          className="absolute w-2 h-2 rounded-full"
          style={{
            left: `${particle.left}%`,
            bottom: '-10px',
            backgroundColor: particle.color,
            animation: `confetti-fall ${particle.duration}s ease-out ${particle.delay}s forwards`,
          }}
        />
      ))}
    </div>
  );
}

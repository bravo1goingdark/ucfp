import { useEffect, useRef } from 'react';

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  size: number;
  opacity: number;
  color: string;
}

export function ParticlesBackground() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const particlesRef = useRef<Particle[]>([]);
  const animationRef = useRef<number | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const resizeCanvas = () => {
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;
    };

    resizeCanvas();
    window.addEventListener('resize', resizeCanvas);

    // Initialize particles
    const particleCount = 25;
    const colors = ['#7c3aed', '#a78bfa', '#06b6d4', '#f43f5e', '#f59e0b'];
    
    particlesRef.current = Array.from({ length: particleCount }, () => ({
      x: Math.random() * canvas.width,
      y: Math.random() * canvas.height,
      vx: (Math.random() - 0.5) * 0.5,
      vy: (Math.random() - 0.5) * 0.5,
      size: Math.random() * 3 + 1,
      opacity: Math.random() * 0.2 + 0.05,
      color: colors[Math.floor(Math.random() * colors.length)]
    }));

    let frameCount = 0;
    const animate = () => {
      frameCount++;
      // Render every 2nd frame for performance
      if (frameCount % 2 === 0) {
        ctx.clearRect(0, 0, canvas.width, canvas.height);

        particlesRef.current.forEach((particle, i) => {
          // Update position
          particle.x += particle.vx;
          particle.y += particle.vy;

          // Bounce off edges
          if (particle.x < 0 || particle.x > canvas.width) particle.vx *= -1;
          if (particle.y < 0 || particle.y > canvas.height) particle.vy *= -1;

          // Draw particle
          ctx.beginPath();
          ctx.arc(particle.x, particle.y, particle.size, 0, Math.PI * 2);
          ctx.fillStyle = particle.color;
          ctx.globalAlpha = particle.opacity;
          ctx.fill();

          // Draw connections (only check every 5th particle for performance)
          if (i % 5 === 0) {
            particlesRef.current.slice(i + 1).forEach((other, j) => {
              if (j % 3 !== 0) return; // Skip most connections
              const dx = particle.x - other.x;
              const dy = particle.y - other.y;
              const distance = Math.sqrt(dx * dx + dy * dy);

              if (distance < 100) {
                ctx.beginPath();
                ctx.moveTo(particle.x, particle.y);
                ctx.lineTo(other.x, other.y);
                ctx.strokeStyle = particle.color;
                ctx.globalAlpha = (1 - distance / 100) * 0.2;
                ctx.lineWidth = 0.5;
                ctx.stroke();
              }
            });
          }
        });

        ctx.globalAlpha = 1;
      }

      animationRef.current = requestAnimationFrame(animate);
    };

    animate();

    return () => {
      window.removeEventListener('resize', resizeCanvas);
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current);
      }
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      className="particles-container"
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        width: '100%',
        height: '100%',
        pointerEvents: 'none',
        zIndex: 0,
      }}
    />
  );
}

export function FloatingShapes() {
  return (
    <div className="floating-shapes" style={{
      position: 'fixed',
      top: 0,
      left: 0,
      width: '100%',
      height: '100%',
      pointerEvents: 'none',
      zIndex: 0,
      overflow: 'hidden',
    }}>
      {/* Animated gradient orbs - reduced opacity */}
      <div className="blob blob-1" style={{
        position: 'absolute',
        top: '10%',
        right: '10%',
        width: '400px',
        height: '400px',
        borderRadius: '50%',
        filter: 'blur(100px)',
        opacity: 0.15,
      }} />
      <div className="blob blob-2" style={{
        position: 'absolute',
        top: '60%',
        left: '5%',
        width: '300px',
        height: '300px',
        borderRadius: '50%',
        filter: 'blur(100px)',
        opacity: 0.12,
      }} />
      <div className="blob blob-3" style={{
        position: 'absolute',
        bottom: '20%',
        right: '30%',
        width: '350px',
        height: '350px',
        borderRadius: '50%',
        filter: 'blur(100px)',
        opacity: 0.1,
      }} />

      {/* Geometric shapes */}
      <div className="float" style={{
        position: 'absolute',
        top: '20%',
        left: '15%',
        width: '60px',
        height: '60px',
        border: '2px solid rgba(124, 58, 237, 0.2)',
        borderRadius: '12px',
        transform: 'rotate(45deg)',
      }} />
      <div className="float-delayed" style={{
        position: 'absolute',
        top: '70%',
        right: '20%',
        width: '40px',
        height: '40px',
        background: 'rgba(6, 182, 212, 0.1)',
        borderRadius: '50%',
      }} />
      <div className="spin-slow" style={{
        position: 'absolute',
        top: '40%',
        right: '10%',
        width: '80px',
        height: '80px',
        border: '2px dashed rgba(244, 63, 94, 0.2)',
        borderRadius: '50%',
      }} />
    </div>
  );
}

export function NoiseOverlay() {
  return <div className="noise-overlay" />;
}

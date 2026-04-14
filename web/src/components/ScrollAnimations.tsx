import { useEffect, useRef, useState, type ReactNode } from 'react'
import { motion, useScroll, useTransform, useSpring, useInView } from 'framer-motion'

interface ScrollRevealProps {
  children: ReactNode
  direction?: 'up' | 'down' | 'left' | 'right' | 'scale'
  delay?: number
  duration?: number
  className?: string
  style?: React.CSSProperties
}

export function ScrollReveal({
  children,
  direction = 'up',
  delay = 0,
  duration = 0.6,
  className = '',
  style,
}: ScrollRevealProps) {
  const ref = useRef(null)
  const isInView = useInView(ref, { once: true, margin: '-60px' })

  const initial = {
    opacity: 0,
    y: direction === 'up' ? 24 : direction === 'down' ? -24 : 0,
    x: direction === 'left' ? 24 : direction === 'right' ? -24 : 0,
    scale: direction === 'scale' ? 0.96 : 1,
  }

  return (
    <motion.div
      ref={ref}
      className={className}
      style={style}
      initial={initial}
      animate={isInView ? { opacity: 1, y: 0, x: 0, scale: 1 } : initial}
      transition={{ duration, delay, ease: [0.16, 1, 0.3, 1] }}
    >
      {children}
    </motion.div>
  )
}

interface StaggerChildrenProps {
  children: ReactNode
  staggerDelay?: number
  className?: string
  style?: React.CSSProperties
}

export function StaggerChildren({
  children,
  staggerDelay = 0.06,
  className = '',
  style,
}: StaggerChildrenProps) {
  const ref = useRef(null)
  const isInView = useInView(ref, { once: true, margin: '-60px' })

  return (
    <motion.div
      ref={ref}
      className={className}
      style={style}
      initial="hidden"
      animate={isInView ? 'visible' : 'hidden'}
      variants={{
        visible: { transition: { staggerChildren: staggerDelay } },
        hidden: {},
      }}
    >
      {children}
    </motion.div>
  )
}

export function StaggerItem({ children, className = '' }: { children: ReactNode; className?: string }) {
  return (
    <motion.div
      className={className}
      variants={{
        hidden: { opacity: 0, y: 16 },
        visible: { opacity: 1, y: 0, transition: { duration: 0.5, ease: [0.16, 1, 0.3, 1] } },
      }}
    >
      {children}
    </motion.div>
  )
}

interface CounterProps {
  end: number
  duration?: number
  suffix?: string
  prefix?: string
  decimals?: number
}

export function AnimatedCounter({ end, duration = 2, suffix = '', prefix = '', decimals = 0 }: CounterProps) {
  const ref = useRef(null)
  const isInView = useInView(ref, { once: true })
  const [count, setCount] = useState(0)

  useEffect(() => {
    if (!isInView) return
    let frame: number
    const startTime = performance.now()

    const animate = (now: number) => {
      const elapsed = now - startTime
      const progress = Math.min(elapsed / (duration * 1000), 1)
      const eased = 1 - Math.pow(1 - progress, 3)
      setCount(eased * end)
      if (progress < 1) {
        frame = requestAnimationFrame(animate)
      }
    }

    frame = requestAnimationFrame(animate)
    return () => cancelAnimationFrame(frame)
  }, [isInView, end, duration])

  return (
    <span ref={ref}>
      {prefix}{decimals > 0 ? count.toFixed(decimals) : Math.round(count)}{suffix}
    </span>
  )
}

interface ParallaxProps {
  children: ReactNode
  speed?: number
  className?: string
  style?: React.CSSProperties
}

export function ParallaxSection({ children, speed = 0.3, className = '', style }: ParallaxProps) {
  const ref = useRef(null)
  const { scrollYProgress } = useScroll({
    target: ref,
    offset: ['start end', 'end start'],
  })

  const y = useTransform(scrollYProgress, [0, 1], [50 * speed, -50 * speed])
  const smoothY = useSpring(y, { stiffness: 100, damping: 30 })

  return (
    <motion.div ref={ref} style={{ y: smoothY, ...style }} className={className}>
      {children}
    </motion.div>
  )
}

export function ScrollProgress() {
  const { scrollYProgress } = useScroll()
  const scaleX = useSpring(scrollYProgress, { stiffness: 100, damping: 30, restDelta: 0.001 })

  return (
    <motion.div
      style={{ scaleX }}
      className="fixed top-0 inset-x-0 h-[2px] bg-accent-600 origin-left z-[9999]"
    />
  )
}

import { useEffect, useState } from 'react'
import { HashRouter, Routes, Route, useLocation } from 'react-router-dom'
import LandingPage from './pages/LandingPage'
import PlaygroundPage from './pages/PlaygroundPage'
import DashboardPage from './pages/DashboardPage'
import Footer from './components/Footer'
import Navigation from './components/Navigation'
import { useDarkMode } from './hooks/useDarkMode'
import { ConfigProvider } from './context/ConfigContext'

function Shell() {
  const [scrolled, setScrolled] = useState(false)
  const { theme, toggleTheme } = useDarkMode()
  const location = useLocation()
  const hideFooter = location.pathname.startsWith('/dashboard')

  useEffect(() => {
    const handleScroll = () => setScrolled(window.scrollY > 50)
    window.addEventListener('scroll', handleScroll)
    return () => window.removeEventListener('scroll', handleScroll)
  }, [])

  return (
    <div className="min-h-screen flex flex-col bg-white dark:bg-zinc-950 text-zinc-900 dark:text-zinc-100">
      <Navigation scrolled={scrolled} theme={theme} toggleTheme={toggleTheme} />
      <main className="flex-1">
        <Routes>
          <Route path="/" element={<LandingPage />} />
          <Route path="/playground" element={<PlaygroundPage />} />
          <Route path="/dashboard" element={<DashboardPage />} />
          <Route path="*" element={<LandingPage />} />
        </Routes>
      </main>
      {!hideFooter && <Footer />}
    </div>
  )
}

export default function App() {
  return (
    <ConfigProvider>
      <HashRouter>
        <Shell />
      </HashRouter>
    </ConfigProvider>
  )
}

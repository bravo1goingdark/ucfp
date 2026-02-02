import { useEffect, useState } from 'react'
import './styles/App.css'
import LandingPage from './pages/LandingPage'
import Footer from './components/Footer'
import Navigation from './components/Navigation'
import { useDarkMode } from './hooks/useDarkMode'

function App() {
  const [scrolled, setScrolled] = useState(false)
  const { theme, toggleTheme } = useDarkMode()

  useEffect(() => {
    const handleScroll = () => {
      setScrolled(window.scrollY > 50)
    }
    window.addEventListener('scroll', handleScroll)
    return () => window.removeEventListener('scroll', handleScroll)
  }, [])

  return (
    <div className="app">
      <Navigation scrolled={scrolled} theme={theme} toggleTheme={toggleTheme} />
      <main>
        <LandingPage />
      </main>
      <Footer />
    </div>
  )
}

export default App

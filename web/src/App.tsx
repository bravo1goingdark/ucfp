import { useEffect, useState } from 'react'
import './App.css'
import Hero from './components/Hero'
import Features from './components/Features'
import Pipeline from './components/Pipeline'
import Modalities from './components/Modalities'
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
        <Hero />
        <Features />
        <Pipeline />
        <Modalities />
      </main>
      <Footer />
    </div>
  )
}

export default App

import { motion } from 'framer-motion'
import { FileText, Image, Music, Video, FileStack, Box, Check, Clock } from 'lucide-react'
import './Modalities.css'

const modalities = [
  { 
    icon: FileText, 
    name: 'Text', 
    status: 'Ready', 
    description: 'Full text support with Unicode normalization and semantic embeddings for comprehensive text fingerprinting.' 
  },
  { 
    icon: Image, 
    name: 'Image', 
    status: 'Planned', 
    description: 'Perceptual hashing and vision-language embeddings for detecting similar and modified images.' 
  },
  { 
    icon: Music, 
    name: 'Audio', 
    status: 'Planned', 
    description: 'Audio fingerprinting with mel-frequency cepstral coefficients for music and speech recognition.' 
  },
  { 
    icon: Video, 
    name: 'Video', 
    status: 'Planned', 
    description: 'Video scene detection and temporal fingerprinting for content identification across frames.' 
  },
  { 
    icon: FileStack, 
    name: 'Document', 
    status: 'Planned', 
    description: 'PDF and document layout understanding with OCR and structural analysis capabilities.' 
  },
  { 
    icon: Box, 
    name: '3D Model', 
    status: 'Planned', 
    description: '3D model fingerprinting for mesh comparison, similarity detection, and asset tracking across libraries.' 
  },
]

export default function Modalities() {
  return (
    <section id="modalities" className="modalities">
      <div className="section-header">
        <h2 className="section-title">
          Multi-<span className="gradient-text">Modal</span> Support
        </h2>
        <p className="section-subtitle">
          From text to 3D models, UCFP handles six content types through a unified fingerprinting pipeline.
        </p>
      </div>

      <div className="modalities-grid">
        {modalities.map((modality, index) => (
          <motion.div
            key={modality.name}
            className="modality-card"
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true, margin: "-50px" }}
            transition={{ duration: 0.4, delay: index * 0.05 }}
          >
            <div className="modality-header">
              <div className="modality-icon">
                <modality.icon size={22} strokeWidth={1.5} />
              </div>
              <span className={`modality-status ${modality.status.toLowerCase()}`}>
                {modality.status === 'Ready' ? <Check size={12} /> : <Clock size={12} />}
                {modality.status}
              </span>
            </div>
            <h3 className="modality-name">{modality.name}</h3>
            <p className="modality-description">{modality.description}</p>
          </motion.div>
        ))}
      </div>
    </section>
  )
}

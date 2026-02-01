# UCFP Web Interface

React-based web interface for the Universal Content Fingerprinting (UCFP) system.

## Overview

This web interface provides a user-friendly frontend for interacting with the UCFP content fingerprinting pipeline, including:

- Document processing and fingerprinting
- Batch upload interface
- Search and matching visualization
- Index management dashboard
- Real-time metrics display

## Quick Start

### Prerequisites

- Node.js 18+ and npm
- UCFP server running (see [`crates/server/README.md`](../crates/server/README.md))

### Development

```bash
# Install dependencies
npm install

# Start development server
npm run dev

# Build for production
npm run build
```

### Environment Configuration

Create a `.env` file:

```env
VITE_API_BASE_URL=http://localhost:8080
VITE_API_KEY=your-api-key
```

---

## API Documentation

The web interface communicates with the UCFP server via REST API. Complete API documentation:

- **Full API Reference**: [`crates/server/API.md`](../crates/server/API.md)
- **Server Setup Guide**: [`crates/server/README.md`](../crates/server/README.md)

### Key API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/process` | POST | Process single document |
| `/api/v1/batch` | POST | Process multiple documents |
| `/api/v1/index/search` | GET | Search indexed documents |
| `/api/v1/match` | POST | Match documents by similarity |

### Example: Processing with Chunking

```typescript
const response = await fetch('http://localhost:8080/api/v1/process', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'X-API-Key': 'your-api-key'
  },
  body: JSON.stringify({
    text: longDocumentContent,
    enable_semantic: true,
    semantic_config: {
      max_sequence_length: 512,
      enable_chunking: true,        // Enable for long documents
      chunk_overlap_ratio: 0.5,     // 50% overlap
      pooling_strategy: 'weighted_mean'
    }
  })
});
```

---

## Architecture

Built with:
- **React 18** - UI framework
- **TypeScript** - Type safety
- **Vite** - Build tool and dev server
- **Tailwind CSS** - Styling

## Development

This template provides a minimal setup to get React working in Vite with HMR and some ESLint rules.

Currently, two official plugins are available:

- [@vitejs/plugin-react](https://github.com/vitejs/vite-plugin-react/blob/main/packages/plugin-react) uses [Babel](https://babeljs.io/) (or [oxc](https://oxc.rs) when used in [rolldown-vite](https://vite.dev/guide/rolldown)) for Fast Refresh
- [@vitejs/plugin-react-swc](https://github.com/vitejs/vite-plugin-react/blob/main/packages/plugin-react-swc) uses [SWC](https://swc.rs/) for Fast Refresh

### Expanding the ESLint configuration

If you are developing a production application, we recommend updating the configuration to enable type-aware lint rules:

```js
export default defineConfig([
  globalIgnores(['dist']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      // Other configs...

      // Remove tseslint.configs.recommended and replace with this
      tseslint.configs.recommendedTypeChecked,
      // Alternatively, use this for stricter rules
      tseslint.configs.strictTypeChecked,
      // Optionally, add this for stylistic rules
      tseslint.configs.stylisticTypeChecked,

      // Other configs...
    ],
    languageOptions: {
      parserOptions: {
        project: ['./tsconfig.node.json', './tsconfig.app.json'],
        tsconfigRootDir: import.meta.dirname,
      },
      // other options...
    },
  },
])
```

You can also install [eslint-plugin-react-x](https://github.com/Rel1cx/eslint-react/tree/main/packages/plugins/eslint-plugin-react-x) and [eslint-plugin-react-dom](https://github.com/Rel1cx/eslint-react/tree/main/packages/plugins/eslint-plugin-react-dom) for React-specific lint rules:

```js
// eslint.config.js
import reactX from 'eslint-plugin-react-x'
import reactDom from 'eslint-plugin-react-dom'

export default defineConfig([
  globalIgnores(['dist']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      // Other configs...
      // Enable lint rules for React
      reactX.configs['recommended-typescript'],
      // Enable lint rules for React DOM
      reactDom.configs.recommended,
    ],
    languageOptions: {
      parserOptions: {
        project: ['./tsconfig.node.json', './tsconfig.app.json'],
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
])
```

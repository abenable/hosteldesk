// Must stay first: hooks React before it loads. Installs nothing in production.
import 'retouche-dev/instrument';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { Retouche } from 'retouche-dev';
import retoucheConfig from '../retouche.config';
import App from './App';
import './index.css';

const root = document.getElementById('root');
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
      <Retouche config={retoucheConfig} />
    </StrictMode>,
  );
}

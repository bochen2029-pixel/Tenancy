import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './styles/globals.css';
import { setupStreamConsumer } from './streaming/streamConsumer';
import { events } from './lib/tauri';
import { useDaveStore } from './state/store';

(async () => {
  try {
    await setupStreamConsumer();
    await events.onInitError((msg) => {
      useDaveStore.getState().setInitError(msg);
    });
  } catch (e) {
    console.error('listener setup failed', e);
  }
  useDaveStore.getState().init();
})();

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

import '@fontsource-variable/bricolage-grotesque/opsz.css';
import './app.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { initBackend } from './lib/api';
import { app } from './lib/store.svelte';

// No top-level await: the macOS/Linux build targets Safari 13.
void initBackend().then(() => {
  mount(App, { target: document.getElementById('app')! });
  void app.init();
});

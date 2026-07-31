import { invoke } from '@tauri-apps/api/core'

const startWithWindows = document.getElementById('start-with-windows');
const startMinimized = document.getElementById('start-minimized');
const feedback = document.getElementById('startup-feedback');

function showFeedback(message, kind) {
  feedback.textContent = message;
  feedback.classList.remove('ok', 'error');
  if (kind) {
    feedback.classList.add(kind);
  }
}

// Registering with Windows can fail on its own, so the checkboxes are only trusted once the
// backend confirms. On failure they are put back to what is actually stored rather than left
// showing a setting that never took effect.
async function save() {
  const preferences = {
    startWithWindows: startWithWindows.checked,
    startMinimized: startMinimized.checked,
  };

  try {
    await invoke('set_startup_preferences', { preferences });
    showFeedback('Preferencias guardadas', 'ok');
  } catch (error) {
    showFeedback(String(error), 'error');
    await load();
  }
}

async function load() {
  try {
    const preferences = await invoke('startup_preferences');
    startWithWindows.checked = preferences.startWithWindows;
    startMinimized.checked = preferences.startMinimized;
  } catch (error) {
    showFeedback('No se pudieron leer las preferencias de inicio', 'error');
    console.error('startup_preferences failed:', error);
  }
}

startWithWindows.addEventListener('change', save);
startMinimized.addEventListener('change', save);

load();

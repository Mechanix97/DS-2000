import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-shell'

const clientIdInput = document.getElementById('discord-client-id');
const clientSecretInput = document.getElementById('discord-client-secret');
const redirectUriLabel = document.getElementById('discord-redirect-uri');
const saveButton = document.getElementById('discord-save-button');
const clearButton = document.getElementById('discord-clear-button');
const guideLink = document.getElementById('discord-setup-guide');
const feedback = document.getElementById('discord-feedback');

let setupGuideUrl = null;

function showFeedback(message, kind) {
  feedback.textContent = message;
  feedback.classList.remove('ok', 'error');
  if (kind) {
    feedback.classList.add(kind);
  }
}

function selectTab(tabName) {
  document.querySelectorAll('.menu-item').forEach(item => {
    item.classList.toggle('active', item.dataset.tab === tabName);
  });
  document.querySelectorAll('.tab-content').forEach(content => {
    content.style.display = content.id === tabName ? 'block' : 'none';
  });
}

// The secret is never returned by the backend once stored, so the field stays empty and shows a
// placeholder instead. Leaving it empty on save keeps the stored secret untouched.
function applyStatus(status) {
  setupGuideUrl = status.setup_guide_url;

  if (status.redirect_uri) {
    redirectUriLabel.textContent = status.redirect_uri;
  }
  if (status.client_id) {
    clientIdInput.value = status.client_id;
  }
  clientSecretInput.placeholder = status.has_client_secret
    ? 'Guardado — dejalo vacío para no cambiarlo'
    : 'Se muestra una sola vez en Discord';

  clearButton.style.display = status.has_client_secret ? 'inline-block' : 'none';

  if (status.connected) {
    showFeedback('Conectado a Discord.', 'ok');
  } else if (status.has_client_secret) {
    showFeedback('Credenciales guardadas. Esperando a Discord…', null);
  }

  return status;
}

async function refreshStatus() {
  try {
    return applyStatus(await invoke('discord_credentials_status'));
  } catch (error) {
    console.error('No se pudo leer el estado de Discord:', error);
    showFeedback('No se pudo leer el estado de Discord.', 'error');
    return null;
  }
}

saveButton.addEventListener('click', async () => {
  const clientId = clientIdInput.value.trim();
  const clientSecret = clientSecretInput.value.trim();

  if (!clientId || !clientSecret) {
    showFeedback('Completá el Client ID y el Client Secret.', 'error');
    return;
  }

  saveButton.disabled = true;
  showFeedback('Guardando…', null);

  try {
    await invoke('discord_set_credentials', { clientId, clientSecret });
    clientSecretInput.value = '';
    showFeedback('Guardado. Autorizá la aplicación en la ventana de Discord.', 'ok');
    await refreshStatus();
  } catch (error) {
    // The backend returns actionable messages, so surface them verbatim.
    showFeedback(String(error), 'error');
  } finally {
    saveButton.disabled = false;
  }
});

clearButton.addEventListener('click', async () => {
  clearButton.disabled = true;
  try {
    await invoke('discord_clear_credentials');
    clientIdInput.value = '';
    clientSecretInput.value = '';
    showFeedback('Aplicación de Discord desvinculada.', null);
    await refreshStatus();
  } catch (error) {
    showFeedback(String(error), 'error');
  } finally {
    clearButton.disabled = false;
  }
});

guideLink.addEventListener('click', async () => {
  if (setupGuideUrl) {
    await open(setupGuideUrl);
  }
});

// Open on the Discord tab when there is nothing configured yet: without credentials the app
// cannot do anything useful, so that is where a new user needs to land.
refreshStatus().then(status => {
  if (status && !status.has_client_secret) {
    selectTab('discord');
  }
});

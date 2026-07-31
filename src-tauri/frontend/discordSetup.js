import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-shell'
import { t, ready, onLanguageChange } from './i18n.js'

const clientIdInput = document.getElementById('discord-client-id');
const clientSecretInput = document.getElementById('discord-client-secret');
const redirectUriLabel = document.getElementById('discord-redirect-uri');
const saveButton = document.getElementById('discord-save-button');
const clearButton = document.getElementById('discord-clear-button');
const guideLink = document.getElementById('discord-setup-guide');
const feedback = document.getElementById('discord-feedback');

let setupGuideUrl = null;
let hasClientSecret = false;

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
  hasClientSecret = status.has_client_secret;
  applySecretPlaceholder();

  clearButton.style.display = status.has_client_secret ? 'inline-block' : 'none';

  if (status.connected) {
    showFeedback(t('discord.connected'), 'ok');
  } else if (status.has_client_secret) {
    showFeedback(t('discord.waiting'), null);
  }

  return status;
}

// Not marked up with data-i18n-placeholder: which of the two placeholders applies depends on
// whether a secret is stored, so the markup-driven pass would overwrite the right one.
function applySecretPlaceholder() {
  clientSecretInput.placeholder = hasClientSecret
    ? t('discord.clientSecretStored')
    : t('discord.clientSecretPlaceholder');
}

async function refreshStatus() {
  try {
    return applyStatus(await invoke('discord_credentials_status'));
  } catch (error) {
    console.error('Could not read the Discord status:', error);
    showFeedback(t('discord.statusFailed'), 'error');
    return null;
  }
}

saveButton.addEventListener('click', async () => {
  const clientId = clientIdInput.value.trim();
  const clientSecret = clientSecretInput.value.trim();

  if (!clientId || !clientSecret) {
    showFeedback(t('discord.missingFields'), 'error');
    return;
  }

  saveButton.disabled = true;
  showFeedback(t('discord.saving'), null);

  try {
    await invoke('discord_set_credentials', { clientId, clientSecret });
    clientSecretInput.value = '';
    showFeedback(t('discord.saved'), 'ok');
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
    showFeedback(t('discord.unlinked'), null);
    await refreshStatus();
  } catch (error) {
    showFeedback(String(error), 'error');
  } finally {
    clearButton.disabled = false;
  }
});

// Discord draws its authorisation modal inside its own window, so with Discord minimised to the
// tray the request is queued and invisible. Without this notice the app just looks stuck.
listen('DISCORD_AWAITING_AUTHORIZATION_EVENT', () => {
  showFeedback(t('discord.awaitingAuthorization'), null);
});

guideLink.addEventListener('click', async () => {
  if (setupGuideUrl) {
    await open(setupGuideUrl);
  }
});

// The placeholder is the only text here that survives a language change: the feedback line
// describes something that just happened, so restating it in another language would be a lie
// about when it happened.
onLanguageChange(applySecretPlaceholder);

// Open on the Discord tab when there is nothing configured yet: without credentials the app
// cannot do anything useful, so that is where a new user needs to land.
ready.then(refreshStatus).then(status => {
  if (status && !status.has_client_secret) {
    selectTab('discord');
  }
});

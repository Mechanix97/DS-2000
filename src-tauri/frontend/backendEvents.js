import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

var mute = false;
var deaf = false;
var discordConnected = false;
var serialConnected = false;

const micIcon = document.getElementById('mic-icon');
const headsetIcon = document.getElementById('headset-icon');
const discordStatus = document.getElementById('discord-status');
const serialStatus = document.getElementById('serial-status');

function updateIcons() {
  console.log('Updating icons:', { mute, deaf });
  // Update mic icon
  if (mute || deaf) {
    micIcon.classList.add('muted');
    console.log('Mic icon classList:', micIcon.classList.toString());
  } else {
    micIcon.classList.remove('muted');
    console.log('Mic icon classList:', micIcon.classList.toString());
  }
  // Update headset icon
  if (deaf) {
    headsetIcon.classList.add('deafened');
    console.log('Headset icon classList:', headsetIcon.classList.toString());
  } else {
    headsetIcon.classList.remove('deafened');
    console.log('Headset icon classList:', headsetIcon.classList.toString());
  }
}

function updateConnectionStatus() {
  // Update Discord status
  if (discordStatus) {
    discordStatus.classList.remove('connected', 'disconnected');
    if (discordConnected) {
      discordStatus.textContent = 'Discord: Conectado';
      discordStatus.classList.add('connected');
    } else {
      discordStatus.textContent = 'Discord: No conectado';
      discordStatus.classList.add('disconnected');
    }
  }
  // Update Serial status
  if (serialStatus) {
    serialStatus.classList.remove('connected', 'disconnected');
    if (serialConnected) {
      serialStatus.textContent = 'Serial: Conectado';
      serialStatus.classList.add('connected');
    } else {
      serialStatus.textContent = 'Serial: No conectado';
      serialStatus.classList.add('disconnected');
    }
  }
  console.log('Connection status updated:', { discordConnected, serialConnected });
}

micIcon.addEventListener('click', () => {
  mute = !mute;
  invoke('ds_set_voice_settings_command', { mute: mute, deaf: deaf });
  console.log('Mic clicked, new mute state:', mute);
  updateIcons();
});

headsetIcon.addEventListener('click', () => {
  deaf = !deaf;
  invoke('ds_set_voice_settings_command', { mute: mute, deaf: deaf });
  console.log('Headset clicked, new deaf state:', deaf);
  updateIcons();
});

listen('DISCORD_VOICE_SETTINGS_EVENT', event => {
  try {
    console.log('Raw payload recibido (voice):', event.payload);
    const payload = JSON.parse(event.payload);
    mute = 'mute' in payload ? Boolean(payload.mute) : mute;
    deaf = 'deafen' in payload ? Boolean(payload.deafen) : deaf;
    console.log('Variables actualizadas:', { mute, deaf });
    updateIcons();
  } catch (error) {
    console.error('Error al parsear el JSON (voice):', error);
    console.error('Payload problemático (voice):', event.payload);
  }
});

listen('DISCORD_CONNECTION_STATUS_EVENT', event => {
  console.log('Raw payload recibido (Discord):', event.payload);
  discordConnected = event.payload === 'true';
  console.log('Discord connection state:', discordConnected ? 'Conectado' : 'No conectado');
  updateConnectionStatus();
});

listen('SERIAL_CONNECTION_STATUS_EVENT', event => {
  console.log('Raw payload recibido (Serial):', event.payload);
  serialConnected = event.payload === 'true';
  console.log('Serial connection state:', serialConnected ? 'Conectado' : 'No conectado');
  updateConnectionStatus();
});

// Update slider values in real-time
document.querySelectorAll('input[type="range"]').forEach(slider => {
  const valueSpan = document.getElementById(slider.id.replace('slider', 'value'));
  valueSpan.textContent = slider.value;
  slider.addEventListener('input', () => {
    valueSpan.textContent = slider.value;
  });
});

// Handle mode selector changes
document.getElementById('mode-selector').addEventListener('change', (event) => {
  const mode = event.target.value;
  const led1Sliders = document.getElementById('rgb-sliders-led1');
  const led2Sliders = document.getElementById('rgb-sliders-led2');

  if (mode === 'ciclar') {
    led1Sliders.style.display = 'none';
    led2Sliders.style.display = 'none';
  } else {
    led1Sliders.style.display = 'block';
    led2Sliders.style.display = 'block';
  }
});

invoke('controller_start');

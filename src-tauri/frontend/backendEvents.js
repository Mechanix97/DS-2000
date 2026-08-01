import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { t, onLanguageChange } from './i18n.js'

var mute = false;
var deaf = false;
var discordConnected = false;
var serialConnected = false;

const micIcon = document.getElementById('mic-icon');
const headsetIcon = document.getElementById('headset-icon');
const discordStatus = document.getElementById('discord-status');
const serialStatus = document.getElementById('serial-status');

const rgbModeSelector = document.getElementById('mode-selector');
const brightnessSlider = document.getElementById('brightness-slider');

// led 1
const led1RedSlider = document.getElementById('led1-red-slider');
const led1GreenSlider = document.getElementById('led1-green-slider');
const led1BlueSlider = document.getElementById('led1-blue-slider');

// led 2
const led2RedSlider = document.getElementById('led2-red-slider');
const led2GreenSlider = document.getElementById('led2-green-slider');
const led2BlueSlider = document.getElementById('led2-blue-slider');

function updateIcons() {
  if (mute || deaf) {
    micIcon.classList.add('muted');
  } else {
    micIcon.classList.remove('muted');
  }
  if (deaf) {
    headsetIcon.classList.add('deafened');
  } else {
    headsetIcon.classList.remove('deafened');
  }
}

function updateConnectionStatus() {
  // Update Discord status
  if (discordStatus) {
    discordStatus.classList.remove('connected', 'disconnected');
    if (discordConnected) {
      discordStatus.textContent = t('status.discordConnected');
      discordStatus.classList.add('connected');
    } else {
      discordStatus.textContent = t('status.discordDisconnected');
      discordStatus.classList.add('disconnected');
    }
  }
  // Update Serial status
  if (serialStatus) {
    serialStatus.classList.remove('connected', 'disconnected');
    if (serialConnected) {
      serialStatus.textContent = t('status.serialConnected');
      serialStatus.classList.add('connected');
    } else {
      serialStatus.textContent = t('status.serialDisconnected');
      serialStatus.classList.add('disconnected');
    }
  }
}

// These labels are written from JavaScript, so the markup-driven pass cannot reach them: they
// have to be rewritten whenever the language changes.
onLanguageChange(updateConnectionStatus);

micIcon.addEventListener('click', () => {
  invoke('ds_set_voice_settings_command', { mute: !mute, deaf: deaf });
  updateIcons();
});

headsetIcon.addEventListener('click', () => {
  invoke('ds_set_voice_settings_command', { mute: mute, deaf: !deaf });
  updateIcons();
});

// These events are serialized by serde, so the payload arrives as a real object or boolean.
// They are also only emitted when the value actually changes, so every one of them is news.
listen('DISCORD_VOICE_SETTINGS_EVENT', event => {
  const payload = event.payload;
  if (!payload || typeof payload !== 'object') {
    console.error('Unexpected voice settings payload:', payload);
    return;
  }
  mute = Boolean(payload.mute);
  deaf = Boolean(payload.deafen);
  updateIcons();
});

listen('DISCORD_CONNECTION_STATUS_EVENT', event => {
  discordConnected = Boolean(event.payload);
  updateConnectionStatus();
});

listen('SERIAL_CONNECTION_STATUS_EVENT', event => {
  serialConnected = Boolean(event.payload);
  updateConnectionStatus();
});

// Update slider values in real-time
document.querySelectorAll('input[type="range"]').forEach(slider => {
  const valueSpan = document.getElementById(slider.id.replace('slider', 'value'));
  valueSpan.textContent = slider.value;
  slider.addEventListener('input', () => {
    valueSpan.textContent = slider.value;
    updateSwatches();
  });
});

const swatches = [
  { element: document.getElementById('led1-swatch'), red: led1RedSlider, green: led1GreenSlider, blue: led1BlueSlider },
  { element: document.getElementById('led2-swatch'), red: led2RedSlider, green: led2GreenSlider, blue: led2BlueSlider },
];

// Three numbers do not read as a colour, so each LED card previews its own.
// Driven by the sliders alone: the device is write-only for RGB, so this shows
// what was asked for rather than pretending to report back what it lit.
function updateSwatches() {
  swatches.forEach(({ element, red, green, blue }) => {
    element.style.backgroundColor = `rgb(${red.value}, ${green.value}, ${blue.value})`;
  });
}

updateSwatches();

// Handle mode selector changes
rgbModeSelector.addEventListener('change', (event) => {
  const mode = event.target.value;
  const led1Sliders = document.getElementById('rgb-sliders-led1');
  const led2Sliders = document.getElementById('rgb-sliders-led2');

  if (mode === 'cycle') {
    led1Sliders.style.display = 'none';
    led2Sliders.style.display = 'none';
  } else {
    led1Sliders.style.display = 'block';
    led2Sliders.style.display = 'block';
  }
  updateRGB();
});

// The mode goes by name. Sending the <select> index coupled the device's behaviour to the
// option order, and the two had already drifted apart.
function updateRGB() {
  invoke('serial_set_rgb', {
    request: {
      mode: rgbModeSelector.value,
      brightness: parseInt(brightnessSlider.value),
      led1: {
        red: parseInt(led1RedSlider.value),
        green: parseInt(led1GreenSlider.value),
        blue: parseInt(led1BlueSlider.value),
      },
      led2: {
        red: parseInt(led2RedSlider.value),
        green: parseInt(led2GreenSlider.value),
        blue: parseInt(led2BlueSlider.value),
      },
    },
  });
}

brightnessSlider.addEventListener('change', (event) => {
  updateRGB();
});

led1RedSlider.addEventListener('change', (event) => {
  updateRGB();
});

led1GreenSlider.addEventListener('change', (event) => {
  updateRGB();
});

led1BlueSlider.addEventListener('change', (event) => {
  updateRGB();
});


led2RedSlider.addEventListener('change', (event) => {
  updateRGB();
});

led2GreenSlider.addEventListener('change', (event) => {
  updateRGB();
});

led2BlueSlider.addEventListener('change', (event) => {
  updateRGB();
});

// A single source of truth for the version: the one Tauri was built with.
invoke('app_version').then(version => {
  const el = document.getElementById('app-version');
  if (el) el.textContent = version;
}).catch(error => console.error('Could not read the version:', error));

invoke('controller_start');

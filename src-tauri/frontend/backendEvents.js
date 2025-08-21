import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

var mute = false;
var deaf = false;

const micIcon = document.getElementById('mic-icon');
const headsetIcon = document.getElementById('headset-icon');

micIcon.addEventListener('click', () => {
  mute = !mute;
  invoke('ds_set_voice_settings_command', { mute: mute, deaf: deaf });
})

headsetIcon.addEventListener('click', () => {
  deaf = !deaf;
  invoke('ds_set_voice_settings_command', { mute: mute, deaf: deaf });
})

invoke('controller_start');
listen('DOWNLOAD_PROGRESS', event => {
  console.log('Evento recibido desde Rust:' + event.payload)
})

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

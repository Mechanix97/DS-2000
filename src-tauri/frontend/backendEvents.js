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

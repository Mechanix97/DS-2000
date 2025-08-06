import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-shell';

var mute = false;
var deaf = false;

const micIcon = document.getElementById('mic-icon');
const headsetIcon = document.getElementById('headset-icon');
const DS2000Button = document.getElementById('DS2000-button');

micIcon.addEventListener('click', () => {
  mute = !mute;
  invoke('ds_set_voice_settings_command', { mute: mute, deaf: deaf });
})

headsetIcon.addEventListener('click', () => {
  deaf = !deaf;
  invoke('ds_set_voice_settings_command', { mute: mute, deaf: deaf });
})


DS2000Button.addEventListener('click', async () => {
  await open('https://mechardo3d.xyz');
});

invoke('controller_start');
listen('DOWNLOAD_PROGRESS', event => {
  console.log('Evento recibido desde Rust:' + event.payload)
})

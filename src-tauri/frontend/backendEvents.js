import { invoke } from '@tauri-apps/api/core'

var mute = false;
var deaf = false;

document.getElementById('mic-icon')?.addEventListener('click', () => {
  mute = !mute;
  invoke('ds_set_voice_settings_command', { mute: mute, deaf: deaf }).then(console.log())
})

document.getElementById('headset-icon')?.addEventListener('click', () => {
  deaf = !deaf;
  invoke('ds_set_voice_settings_command', { mute: mute, deaf: deaf }).then(console.log())
})

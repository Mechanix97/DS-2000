import { invoke } from '@tauri-apps/api/core'

document.getElementById('discord-button')?.addEventListener('click', () => {
  invoke('hacer_algo', { nombre: 'Lucas' }).then(console.log())
})

import { open } from '@tauri-apps/plugin-shell';

const DS2000Button = document.getElementById('DS2000-button');
const MechardoLabsButton = document.getElementById('Mechardo-Labs-button');

DS2000Button.addEventListener('click', async () => {
    await open('https://mechardo3d.xyz/ds2000');
});

MechardoLabsButton.addEventListener('click', async () => {
    await open('https://mechardo3d.xyz');
});

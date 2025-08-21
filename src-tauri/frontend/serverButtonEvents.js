import { open } from '@tauri-apps/plugin-shell';

const DS2000Button = document.getElementById('DS2000-button');
const MechardoLabsButton = document.getElementById('Mechardo-Labs-button');
const instagramButton = document.getElementById('instagram-button');
const twitterButton = document.getElementById('twitter-button');
const youtubeButton = document.getElementById('youtube-button');
const DiscordInviteButton = document.getElementById('discord-invite-button');
const ServiceTermsButton = document.getElementById('serviceTerms');
const PrivacyPolicyButton = document.getElementById('privacyPolicy');

DS2000Button.addEventListener('click', async () => {
    await open('https://mechardo3d.xyz/ds2000');
});

MechardoLabsButton.addEventListener('click', async () => {
    await open('https://mechardo3d.xyz');
});

instagramButton.addEventListener('click', async () => {
    await open('https://www.instagram.com/mechardo3d/');
});

twitterButton.addEventListener('click', async () => {
    await open('https://x.com/MechardoLabs');
});

youtubeButton.addEventListener('click', async () => {
    await open('https://www.youtube.com/@MechardoLabs');
});

DiscordInviteButton.addEventListener('click', async () => {
    await open('https://discord.gg/VtbFAGJe86');
});

ServiceTermsButton.addEventListener('click', async () => {
    await open('https://www.mechardo3d.xyz/ds2000/terms-of-service');
});

PrivacyPolicyButton.addEventListener('click', async () => {
    await open('https://www.mechardo3d.xyz/ds2000/privacy-policy');
});

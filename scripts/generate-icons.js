import sharp from 'sharp';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const iconsDir = path.join(__dirname, '../src-tauri/icons');

// Ensure icons directory exists
if (!fs.existsSync(iconsDir)) {
  fs.mkdirSync(iconsDir, { recursive: true });
}

// Create icon with lucide-react Cloud icon (matching login screen)
async function createCloudIcon(size) {
  // Calculate padding and scaling for the cloud icon
  // The cloud icon from lucide-react has a 24x24 viewBox
  // Cloud center is approximately (13.25, 14.5) in 24x24 space
  const padding = size * 0.125; // Same as background padding
  const iconSize = size * 0.75; // Available space for icon
  const centerX = size / 2; // Center of the entire canvas
  const centerY = size / 2; // Center of the entire canvas
  const scale = (iconSize * 0.6) / 24; // Scale to use ~60% of available space
  const cloudCenterX = 13.25; // Cloud icon center X in 24x24 space
  const cloudCenterY = 14.5; // Cloud icon center Y in 24x24 space
  
  const svg = `
    <svg width="${size}" height="${size}" xmlns="http://www.w3.org/2000/svg">
      <!-- Background: gray-900 rounded square (matching login screen) -->
      <rect x="${padding}" y="${padding}" 
            width="${iconSize}" height="${iconSize}" 
            rx="${size * 0.125}" ry="${size * 0.125}" fill="#111827"/>
      
      <!-- Cloud icon from lucide-react (properly centered) -->
      <!-- Transform: translate to center, scale, then translate back by icon center offset -->
      <g transform="translate(${centerX}, ${centerY}) scale(${scale}) translate(-${cloudCenterX}, -${cloudCenterY})">
        <path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z" fill="#FFFFFF" stroke="none"/>
      </g>
    </svg>
  `;

  return sharp(Buffer.from(svg))
    .resize(size, size)
    .png();
}

async function generateIcons() {
  const sizes = [
    { size: 32, name: '32x32.png' },
    { size: 128, name: '128x128.png' },
    { size: 256, name: '128x128@2x.png' },
    { size: 512, name: '512x512.png' },
    { size: 1024, name: 'icon.png' }
  ];

  console.log('Generating icons with lucide-react Cloud icon (properly centered)...');
  
  for (const { size, name } of sizes) {
    const icon = await createCloudIcon(size);
    const outputPath = path.join(iconsDir, name);
    await icon.toFile(outputPath);
    console.log(`✓ Created ${name} (${size}x${size})`);
  }

  console.log(`\nAll icons saved to: ${iconsDir}`);
}

generateIcons().catch(console.error);
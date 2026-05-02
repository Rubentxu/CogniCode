// Clean: Using async file operations
const fs = require('fs').promises;

async function readConfig() {
    const data = await fs.readFile('config.json', 'utf8');
    return JSON.parse(data);
}

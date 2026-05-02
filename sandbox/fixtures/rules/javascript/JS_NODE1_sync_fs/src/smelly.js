// Smelly: Using synchronous file operations
const fs = require('fs');

function readConfig() {
    const data = fs.readFileSync('config.json', 'utf8');
    return JSON.parse(data);
}

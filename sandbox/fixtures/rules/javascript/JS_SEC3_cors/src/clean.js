// Clean: CORS with specific origin
const express = require('express');
const app = express();

app.use((req, res, next) => {
    res.header('Access-Control-Allow-Origin', 'https://trusted.example.com');
    next();
});

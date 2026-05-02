// Smelly: Using innerHTML with user input - XSS vulnerability
function displayContent(div, userInput) {
    div.innerHTML = userInput;
}

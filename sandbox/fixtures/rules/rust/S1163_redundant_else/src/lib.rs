/// Function with redundant else - triggers S1163
pub fn test() {
    return;
    else {}
}

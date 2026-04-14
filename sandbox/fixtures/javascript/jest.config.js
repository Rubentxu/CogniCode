module.exports = {
  testEnvironment: 'node',
  roots: ['.'],
  testMatch: ['**/test_*.js'],
  moduleFileExtensions: ['js', 'json'],
  collectCoverageFrom: ['hello.js'],
  coverageDirectory: 'coverage',
  verbose: true,
};

module.exports = {
  preset: 'ts-jest',
  testEnvironment: 'node',
  roots: ['.'],
  testMatch: ['**/test_*.ts'],
  moduleFileExtensions: ['ts', 'tsx', 'js', 'json'],
  collectCoverageFrom: ['hello.ts'],
  coverageDirectory: 'coverage',
  verbose: true,
};

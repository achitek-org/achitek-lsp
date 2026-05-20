# Changelog

## [Unreleased]

## [0.2.0](https://github.com/achitek-org/achitek-ls/compare/achitek-ls-v0.1.1...achitek-ls-v0.2.0)

### ⛰️ Features


- *(ls)* Add tera diagnostic styling & capture prompt variables in file paths - ([3bf5007](https://github.com/achitek-org/achitek-ls/commit/3bf50075bc0fe9efebd666a7315de30658e3a42a))
- *(ls)* Implement first pass at tera code actions - ([d947886](https://github.com/achitek-org/achitek-ls/commit/d94788683436003b19394d390155ad0fb07b6a0d))
- *(ls)* Implement rename - ([accc384](https://github.com/achitek-org/achitek-ls/commit/accc3840cbca98e85ec93662c26c3d4af55c0d14))
- *(ls)* Implemenet go-to-definition from tera template to achitekfile - ([51831e6](https://github.com/achitek-org/achitek-ls/commit/51831e6c6b25dfa7c7fe072cdfacdac74b3ed63c))
- Introduce workspace - ([757e661](https://github.com/achitek-org/achitek-ls/commit/757e6617cc7402b96fb11737b778f9b42bcf7799))

### 🐛 Bug Fixes


- *(ls)* Dismiss tera builtins as diagnostic errors - ([81fc0df](https://github.com/achitek-org/achitek-ls/commit/81fc0df6107b5eed55faaebf4f465849beb89d4b))

### 🚜 Refactor


- *(ls)* Remove code-actions - ([417ca00](https://github.com/achitek-org/achitek-ls/commit/417ca00a75499891b05cd74eb21655b78c525587))
- *(ls)* Split editor features into modules - ([c4d900e](https://github.com/achitek-org/achitek-ls/commit/c4d900ec837bc5d0d9fcff6e94adeed670e1b1b7))
- *(ls)* Use achitekfile parser types - ([ed0cfbd](https://github.com/achitek-org/achitek-ls/commit/ed0cfbdf711b413dac03127ff856d9c4a7e7a8b0))
- *(terafile)* Consolidate vendored grammar ownership - ([d386b4c](https://github.com/achitek-org/achitek-ls/commit/d386b4c44de641b69e80a77d587aa7b4ff02c3b5))

### ⚙️ Miscellaneous Tasks


- *(ls)* Clean up diagnostic publishing in handlers - ([3e028e2](https://github.com/achitek-org/achitek-ls/commit/3e028e296a0e27e196f8f99718d17150d5264398))
- *(ls)* Refactor hover and completion editor features into separate module - ([dec12af](https://github.com/achitek-org/achitek-ls/commit/dec12af2bc9c3932195b4a29bb2346345b59d495))
- *(ls)* Begin using achitefile analysis semantic model - ([df7c340](https://github.com/achitek-org/achitek-ls/commit/df7c34015e10752497b2d1f5de01060ae2327409))
- *(ls)* Implement project diagnostics - ([335c62e](https://github.com/achitek-org/achitek-ls/commit/335c62e430a6aa1656a5dd456bd0cebf4381e407))
- *(ls)* Implement ProjectContext that owns the repeated project lookup/source-loading behavior - ([8ac91a1](https://github.com/achitek-org/achitek-ls/commit/8ac91a127fbf517aa7ba82e3b9706058b0bbc1c2))
- Pass server state to request handlers - ([3ac50b0](https://github.com/achitek-org/achitek-ls/commit/3ac50b0abd47985f3141bdc2ecbb10c4ab89b0e6))
- Update workspace - ([4d67daf](https://github.com/achitek-org/achitek-ls/commit/4d67daf8ded392abaa60ec332a2ee10f3fa4e84b))


## [0.1.1](https://github.com/achitek-org/achitek-ls/compare/v0.1.0...v0.1.1)

### 🐛 Bug Fixes


- Default to stdio if communications channel not provided - ([c69e8d3](https://github.com/achitek-org/achitek-ls/commit/c69e8d3aa832da603dc6ed9a6588f0d812c8b658))

### ⚙️ Miscellaneous Tasks


- Lint - ([b1a5e2d](https://github.com/achitek-org/achitek-ls/commit/b1a5e2da3d419f8ed0cf37087a3e49fde040c0d6))
- Update github token for release-plz release - ([537e241](https://github.com/achitek-org/achitek-ls/commit/537e241f58aad789c60e4068c934feb3cf2e7964))


## [0.1.0]

### ⛰️ Features


- Implement server ([#9](https://github.com/achitek-org/achitek-ls/pull/9)) - ([dd6568c](https://github.com/achitek-org/achitek-ls/commit/dd6568c63a83177b827a4eb9a2f08fe7488205d7))
- Implement command line interface using lexopt ([#8](https://github.com/achitek-org/achitek-ls/pull/8)) - ([cd5167b](https://github.com/achitek-org/achitek-ls/commit/cd5167ba774346da7ae784b3f30c49d96a4714af))
- Initial Achitek LSP - ([a8148b3](https://github.com/achitek-org/achitek-ls/commit/a8148b31af3534c54f5a5dac6bbae2d6db9c9508))

### 📚 Documentation


- Improve documentation - ([6a5030f](https://github.com/achitek-org/achitek-ls/commit/6a5030f1cac4457054a75583ad202b11a1af6c5c))

### ⚙️ Miscellaneous Tasks


- Implement initial ci/cd automation ([#10](https://github.com/achitek-org/achitek-ls/pull/10)) - ([60527e0](https://github.com/achitek-org/achitek-ls/commit/60527e0ed5b91f5250cca9058300c8c43a4b279c))
- Addressing cargo clippy fix recommendations - ([1526709](https://github.com/achitek-org/achitek-ls/commit/1526709b772d4daf883e5da1b678ede4047cea7f))
- Refactor from workspace to single binary and lib structure ([#6](https://github.com/achitek-org/achitek-ls/pull/6)) - ([5493801](https://github.com/achitek-org/achitek-ls/commit/54938016edc3b2747753bf189604e975ad066341))
- Clean up - ([6567de0](https://github.com/achitek-org/achitek-ls/commit/6567de0ea724d91ff622e6e5a1f3fecda18705ed))
- Install deps - ([b2ee3b9](https://github.com/achitek-org/achitek-ls/commit/b2ee3b910a11d85488a5df15ce6bac5587fba298))


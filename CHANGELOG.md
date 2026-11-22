# Changelog
All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

- - -
## [v0.3.0](https://github.com/batleforc/weebo-kura/compare/665d867df22e7d0ceefdf97bff3f64288c6a66da..v0.3.0) - 2026-11-22
#### Features
- (**chart**) hostAliases, extra volumes, Argo CD sync waves, PostgreSQL without CloudNativePG - ([a0125fc](https://github.com/batleforc/weebo-kura/commit/a0125fc278db40e7b6d3c1c2c8ec24757e38222d)) - Max Batleforc, Claude Opus 5.5
#### Miscellaneous Chores
- (**version**) merge v0.2.1 - ([665d867](https://github.com/batleforc/weebo-kura/commit/665d867df22e7d0ceefdf97bff3f64288c6a66da)) - Maxime

- - -

## [v0.2.1](https://github.com/batleforc/weebo-kura/compare/a77de5da6123cc4cc87edf530060868c9662b66d..v0.2.1) - 2026-11-22
#### Bug Fixes
- (**cli**) kura watch no longer drops an edit made while it syncs - ([a77de5d](https://github.com/batleforc/weebo-kura/commit/a77de5da6123cc4cc87edf530060868c9662b66d)) - Max Batleforc, Claude Opus 5.5
#### Miscellaneous Chores
- (**version**) merge v0.2.0 - ([667a89f](https://github.com/batleforc/weebo-kura/commit/667a89f599a2a1af9e64e96c401e993b30a8e7a5)) - Maxime

- - -

## [v0.2.0](https://github.com/batleforc/weebo-kura/compare/d758347b349fabefd86586cd93cb46d2a6af5418..v0.2.0) - 2026-11-22
#### Features
- (**api**) KuraInstance and the operator's reason codes (RFC 0009 phase 0) - ([32da4a6](https://github.com/batleforc/weebo-kura/commit/32da4a679d3db4508d1da311b0c3ef2b9655737a)) - Max Batleforc, Claude Opus 5.5
- (**deploy**) kura-operator's chart and image, the kura chart's operator mode (RFC 0009 phase 3) - ([e88fc2b](https://github.com/batleforc/weebo-kura/commit/e88fc2b8e5d9a678ee5927251f89945755e82ac4)) - Max Batleforc, Claude Opus 5.5
- (**operator**) release, capacity, perf budget, kind proof script; RFC 0009 implemented - ([9a9776d](https://github.com/batleforc/weebo-kura/commit/9a9776dcb2efdf9fa09c4f0e52084a0c4d5ccad3)) - Max Batleforc, Claude Opus 5.5
- (**operator**) kura-operator, the push of the whole set (RFC 0009 phase 2) - ([98f50fb](https://github.com/batleforc/weebo-kura/commit/98f50fb557492954290df8fe2a5ea99d192e411e)) - Max Batleforc, Claude Opus 5.5
- (**server**) PUT /v1/admin/documents, kura-operator's push (RFC 0009 phase 1) - ([9a2e79c](https://github.com/batleforc/weebo-kura/commit/9a2e79c8f51e79c7d1375662b8b3c6eeee1abfbf)) - Max Batleforc, Claude Opus 5.5
#### Bug Fixes
- (**deps**) bump url to 2.5.9 for RUSTSEC-2026-0592 - ([27ec94a](https://github.com/batleforc/weebo-kura/commit/27ec94a6c19c38144a01b3c597d9ac70f2900a63)) - Max Batleforc, Claude Opus 5.5
#### Documentation
- (**rfc**) the implementation plan of RFC 0009, the Kura operator - ([67c3145](https://github.com/batleforc/weebo-kura/commit/67c3145a6676d7099977d75712fa5124438cd1d2)) - Max Batleforc, Claude Opus 5.5
#### Tests
- (**server**) the scoped-token gate knows the documents route's 409 - ([38a3941](https://github.com/batleforc/weebo-kura/commit/38a3941fdd352c453a9b8161042793a353715343)) - Max Batleforc, Claude Opus 5.5
- 86% line gate on every component, the operator's binary under envtest - ([be175ab](https://github.com/batleforc/weebo-kura/commit/be175ab46b45608d76c224d6c3a49bf7b169196e)) - Max Batleforc, Claude Opus 5.5
#### Continuous Integration
- keep the operator's crates out of the per-crate coverage gates - ([af2e1c6](https://github.com/batleforc/weebo-kura/commit/af2e1c6491b3769346c59f60a488cc6e89fe91f4)) - Max Batleforc, Claude Opus 5.5
#### Miscellaneous Chores
- (**version**) merge v0.1.0 - ([d758347](https://github.com/batleforc/weebo-kura/commit/d758347b349fabefd86586cd93cb46d2a6af5418)) - Maxime

- - -

## [v0.1.0](https://github.com/batleforc/weebo-kura/compare/v0.1.0-rc.1..v0.1.0) - 2026-11-21
#### Documentation
- (**rfc**) RFC 0009, the Kura operator (Accepted) - ([7abbf6e](https://github.com/batleforc/weebo-kura/commit/7abbf6e9430827755743f8d6e8733ec6020776fa)) - Max Batleforc, Claude Opus 5.5
- the Fumadocs site on @weebo/operator-docs, and the CRDs of KuraTenant and KuraVault - ([eaf76d8](https://github.com/batleforc/weebo-kura/commit/eaf76d8705c3cfd38eb47c19317c959f9d9b2cd9)) - Max Batleforc, Claude Opus 5.5

- - -

## [v0.1.0-rc.1](https://github.com/batleforc/weebo-kura/compare/bc8e20a8d219f4411cfa01b7de8cccd55457fdd4..v0.1.0-rc.1) - 2026-11-21
#### Features
- (**cli**) add kura verify and the pre-push migration check (RFC 0001 phase 9) - ([7ed3667](https://github.com/batleforc/weebo-kura/commit/7ed3667adbdaf454380c5632cb5b40ac89d12449)) - Max Batleforc, Claude Opus 5.5
- (**dev**) local stack with OpenBao, Authentik and PostgreSQL - ([2fc19eb](https://github.com/batleforc/weebo-kura/commit/2fc19eb0b5727dad24ab8eb537a9ecd2061e45c3)) - Max Batleforc, Claude Opus 5.5
- (**perf**) measure chunking and compression on a copy of a vault - ([3343ff9](https://github.com/batleforc/weebo-kura/commit/3343ff9ae559160ed23f42fd7d2e6d4c28ec3148)) - Max Batleforc, Claude Opus 5.5
- (**ui**) Mermaid diagrams in notes, and images up to 8 MiB - ([703a937](https://github.com/batleforc/weebo-kura/commit/703a937db5a899a4a5b0de85ae933e8d83522493)) - Max Batleforc, Claude Opus 5.5
- (**ui**) a folder with a note of its name opens it - ([833a813](https://github.com/batleforc/weebo-kura/commit/833a8130eb2b49a101a1f5803632bc8b8ff7828c)) - Max Batleforc, Claude Opus 5.5
- (**ui**) the folder tree's width, set with a handle beside it - ([bc54f05](https://github.com/batleforc/weebo-kura/commit/bc54f05780a405280e0a39823f70d4fde0339d60)) - Max Batleforc, Claude Opus 5.5
- (**ui**) the reader's history as a drawer, search in the tree, and a bilingual plugin - ([51ff2d3](https://github.com/batleforc/weebo-kura/commit/51ff2d3e1b0ae3538aee0625c719fc0b656d0dc3)) - Max Batleforc, Claude Opus 5.5
- (**ui**) a vault-centred console, its help, and the Obsidian plugin from it - ([35ac227](https://github.com/batleforc/weebo-kura/commit/35ac2275e813a1c100fc9fe6822aaffd6783c66c)) - Max Batleforc, Claude Opus 5.5
- (**ui**) notification rules and deliveries in the console (RFC 0003 phase 4) - ([a7e8e67](https://github.com/batleforc/weebo-kura/commit/a7e8e678e1ae166578a0198bf33f70322e5661e4)) - Max Batleforc, Claude Opus 5.5
- (**ui**) the back office of the web console (RFC 0003 phase 2) - ([6383e66](https://github.com/batleforc/weebo-kura/commit/6383e66106ac53c3dda4013b66b09c49fd9dcb98)) - Max Batleforc, Claude Opus 5.5
- the synced Obsidian configuration in the console (RFC 0008) - ([80cf466](https://github.com/batleforc/weebo-kura/commit/80cf4666113261ea3b04939da6e4c129686fbe70)) - Max Batleforc, Claude Opus 5.5
- folders sync as entries, empty ones included (RFC 0007) - ([e081be6](https://github.com/batleforc/weebo-kura/commit/e081be643aa104fd44d3b2c5f94c566bfc18fcb4)) - Max Batleforc, Claude Opus 5.5
- name device owners, and prove RFC 0006 in the browser - ([69682bb](https://github.com/batleforc/weebo-kura/commit/69682bbbd15027b5ddcf5055947bf09e01b1f2de)) - Max Batleforc, Claude Opus 5.5
- the last run's counts in the console, and RFC 0006 proven on the stack - ([1cd2cd7](https://github.com/batleforc/weebo-kura/commit/1cd2cd7351f42d80011787d1fa142f032c21ac01)) - Max Batleforc, Claude Opus 5.5
- where each device stands on a vault (RFC 0006) - ([807ead9](https://github.com/batleforc/weebo-kura/commit/807ead9162a696410ec9753cc42a02ee381d6deb)) - Max Batleforc, Claude Opus 5.5
- close the landing gaps of RFC 0003 and 0005, both Implemented - ([32d0bc0](https://github.com/batleforc/weebo-kura/commit/32d0bc0c2e1d31017d4cfc0fd91e5b098a66416d)) - Max Batleforc, Claude Opus 5.5
- batched uploads (RFC 0002 phase 3) - ([34fc574](https://github.com/batleforc/weebo-kura/commit/34fc5741119c14ac71e089906cf84ecd052fa88f)) - Max Batleforc, Claude Opus 5.5
- close the remaining gaps of RFC 0003, 0004 and 0005 - ([49e1b7d](https://github.com/batleforc/weebo-kura/commit/49e1b7d70bd34e62097f03832d87b88e6fc97881)) - Max Batleforc, Claude Opus 5.5
- webhook notifications, server side (RFC 0003 phase 3) - ([62cdb46](https://github.com/batleforc/weebo-kura/commit/62cdb46de27dac26d4aab5f6b511a3c8d957af11)) - Max Batleforc, Claude Opus 5.5
- folder-restricted viewers and the web reader (RFC 0005) - ([c2449ce](https://github.com/batleforc/weebo-kura/commit/c2449ce6df4eed142cff29c9559197b1bf17a63f)) - Max Batleforc, Claude Opus 5.5
- folder-scoped write tokens (RFC 0004) and the web console, phase 1 (RFC 0003) - ([b1854d5](https://github.com/batleforc/weebo-kura/commit/b1854d5f7034b596ac214223985d2d7ea8a7f30a)) - Max Batleforc, Claude Opus 5.5
- Mise en place de nouvelle RFC pour les évolution - ([a4d3ddd](https://github.com/batleforc/weebo-kura/commit/a4d3ddd6ed03d6747764df3d816d49c152c1d184)) - Max Batleforc
- batched chunk and manifest downloads (RFC 0002 phases 1 and 2) - ([1647256](https://github.com/batleforc/weebo-kura/commit/1647256ce6e6c2f5f1bc1376ce3412968bf86705)) - Max Batleforc, Claude Opus 5.5
- implement RFC 0001 phase 8 Obsidian configuration sync - ([ee747b1](https://github.com/batleforc/weebo-kura/commit/ee747b15d41dd8e904bb96b1d28b61b3f421fe9a)) - Max Batleforc, Claude Opus 5.5
- implement RFC 0001 phase 7 scoped tokens and opt-in search - ([1dd8b34](https://github.com/batleforc/weebo-kura/commit/1dd8b34061bf12d3eb2ba5ccb6b6a17361670bb9)) - Max Batleforc, Claude Opus 5.5
- implement RFC 0001 phase 6 operations, load tests and fixes - ([e95719d](https://github.com/batleforc/weebo-kura/commit/e95719dd1fab41c5ef675a4a3fe706dff676a288)) - Max Batleforc, Claude Opus 5.5
- implement RFC 0001 phase 5 history, retention and full resync - ([f19567c](https://github.com/batleforc/weebo-kura/commit/f19567cd0bb07cc46ce55008309b098770768594)) - Max Batleforc, Claude Opus 5.5
- implement RFC 0001 phase 4 merge, notifications and plugin - ([5af5497](https://github.com/batleforc/weebo-kura/commit/5af549745ab70c036ca4a6e05b6eba870b025abf)) - Max Batleforc, Claude Opus 5.5
- implement RFC 0001 phase 3 sync through kura-cli - ([0621b61](https://github.com/batleforc/weebo-kura/commit/0621b6120289bde8fece60cdadcf681216b0dd94)) - Max Batleforc, Claude Opus 5.5
- implement RFC 0001 phase 1 and phase 2 storage - ([b8e3c31](https://github.com/batleforc/weebo-kura/commit/b8e3c311fb4da8cb99bbe57f01f8bf370fb001d2)) - Max Batleforc, Claude Opus 5.5
- init project - ([64c35ef](https://github.com/batleforc/weebo-kura/commit/64c35efe04b5c04312ec0fdf1ab44a2c32af68b0)) - Max Batleforc
#### Bug Fixes
- (**auth**) follow a session through refreshed tokens by auth_time - ([89b2822](https://github.com/batleforc/weebo-kura/commit/89b28228146ad7663254415b517408cec179c44b)) - Max Batleforc, Claude Opus 5.5
- (**events**) re-check the vault role on open sockets - ([85e4fe9](https://github.com/batleforc/weebo-kura/commit/85e4fe9251ace3d0fa1865313b33b3720269e2c2)) - Max Batleforc, Claude Opus 5.5
- (**plugin**) turn a synced plugin off under every id it runs as - ([e1cd3fd](https://github.com/batleforc/weebo-kura/commit/e1cd3fd5e7f559247295d0e4b8bd45930cec12b5)) - Max Batleforc, Claude Opus 5.5
- (**plugin**) no epoch=undefined when the core has no epoch yet - ([36275df](https://github.com/batleforc/weebo-kura/commit/36275df56659f2f29b7a535260cf7fc0224b2b47)) - Max Batleforc, Claude Opus 5.5
- (**plugin**) keep the session across a vault switch, and offer sign-in again - ([15f99e0](https://github.com/batleforc/weebo-kura/commit/15f99e078665df85db24c8f53d9a3767dae92ebe)) - Max Batleforc, Claude Opus 5.5
- (**ui**) the critique's leftovers: status in words, ink for selection, no band - ([47000ba](https://github.com/batleforc/weebo-kura/commit/47000ba87c7e445e59d97992ef8d99fc9259a51c)) - Max Batleforc, Claude Opus 5.5
- (**ui**) the critique of 2026-11-18, from 26/40: names, phones, roles, words - ([d90ea51](https://github.com/batleforc/weebo-kura/commit/d90ea5108ad3f044f2e21b08c7d4db87760ae87d)) - Max Batleforc, Claude Opus 5.5
- resolve the findings of the 2026-11-21 project review - ([7225516](https://github.com/batleforc/weebo-kura/commit/72255166c9a33e3e91a93a01c0fd69f57888d714)) - Max Batleforc, Claude Opus 5.5
- close the 2026-11-15 global review - ([d1bad91](https://github.com/batleforc/weebo-kura/commit/d1bad9198b2295c25d1ab91d4500a535bb660e62)) - Max Batleforc, Claude Opus 5.5
- nine more findings of the 2026-11-15 global review - ([eb6ee24](https://github.com/batleforc/weebo-kura/commit/eb6ee2445459b1611168e251ec7f42eda9733d90)) - Max Batleforc, Claude Opus 5.5
- four findings of the 2026-11-15 global review - ([649579a](https://github.com/batleforc/weebo-kura/commit/649579aae083f62a2b4ab0c290353d9452c562d4)) - Max Batleforc, Claude Opus 5.5
#### Performance Improvements
- (**server**) list vaults in one transaction per tenant and fetch keys unlocked - ([fabe6d1](https://github.com/batleforc/weebo-kura/commit/fabe6d1b7d750a77b6d16c8080faa215fa4bcf8d)) - Max Batleforc, Claude Opus 5.5
#### Documentation
- (**rfc**) PR 9.2 done, phase 2 closed - ([4f8fe94](https://github.com/batleforc/weebo-kura/commit/4f8fe9403d13e0dc99a8089643168341a3c0db73)) - Max Batleforc, Claude Opus 5.5
- (**rfc**) PR 2.6 measured, decisions 52 and 38 kept - ([a423ec6](https://github.com/batleforc/weebo-kura/commit/a423ec641b3073137c15e84dd58e1b24a4a93cf3)) - Max Batleforc, Claude Opus 5.5
- (**rfc**) land RFC 0002 and 0004, revise 0003 and 0005 against the tree - ([1c22fce](https://github.com/batleforc/weebo-kura/commit/1c22fcec4c5b1a264eb32eec95bfb539049c2588)) - Max Batleforc, Claude Opus 5.5
- (**rfc**) draft RFC 0002 batched chunk and manifest transfers - ([37bc868](https://github.com/batleforc/weebo-kura/commit/37bc868e8ada45ee89b93ff119109f50364e32a2)) - Max Batleforc, Claude Opus 5.5
- (**rfc**) accept RFC 0001 kura sync backend - ([a7eca77](https://github.com/batleforc/weebo-kura/commit/a7eca776de8b86916442ff8c1a13343f69bb0876)) - Max Batleforc, Claude Opus 5.5
- (**rfc**) review and settle RFC 0001 kura sync backend - ([1d37ae3](https://github.com/batleforc/weebo-kura/commit/1d37ae3d9f42d63836e889bcc0954eab63ab0f57)) - Max Batleforc, Claude Opus 5.5
- keep the console's design critique - ([8971e9b](https://github.com/batleforc/weebo-kura/commit/8971e9b84bbcfb726f44dd940166b168d1b2bcbd)) - Max Batleforc, Claude Opus 5.5
- add RFC index with planned RFCs - ([d2d3f89](https://github.com/batleforc/weebo-kura/commit/d2d3f89d6eee703679ec7b943d670858a2962d93)) - Max Batleforc, Claude Opus 5.5
#### Tests
- (**heavy**) RFC 0002 observation 3, and an Android emulator for the plugin - ([2583a18](https://github.com/batleforc/weebo-kura/commit/2583a18aebc94f130757d0fdc10270d76b02c044)) - Max Batleforc, Claude Opus 5.5
#### Continuous Integration
- (**release**) tag-triggered release of the image, chart, CLI and plugin - ([f6b031d](https://github.com/batleforc/weebo-kura/commit/f6b031d3ff7ca842bad69a7feafde607331be87f)) - Max Batleforc, Claude Opus 5.5
- (**security**) daily scanners, SECURITY.md, scan the rendered chart - ([59f5dc7](https://github.com/batleforc/weebo-kura/commit/59f5dc79802f7ef4fab04cc01068a7013f760389)) - Max Batleforc, Claude Opus 5.5
- give cog verify a git identity on the runner - ([9687e64](https://github.com/batleforc/weebo-kura/commit/9687e644f5815f7bb7a47c91af338327bb29e3d1)) - Max Batleforc, Claude Opus 5.5
- stop .gitleaksignore quoting the placeholder it ignores - ([9b90870](https://github.com/batleforc/weebo-kura/commit/9b908708fae8e303409babefb3344bcea5de1f0d)) - Max Batleforc, Claude Opus 5.5
- allow a run by hand - ([a76e6df](https://github.com/batleforc/weebo-kura/commit/a76e6df6a334099bc4f9277458d13e26e199c199)) - Max Batleforc, Claude Opus 5.5
- unblock the checks: a mock token in RFC 0004, picomatch in Trivy - ([ac68e90](https://github.com/batleforc/weebo-kura/commit/ac68e90293ef38fc0aea8fa9aec1def7935377c8)) - Max Batleforc, Claude Opus 5.5
#### Refactoring
- (**db**) squash the migrations into 0001_init - ([f53eca6](https://github.com/batleforc/weebo-kura/commit/f53eca6569b15fa1bfba5adba478300a2a5ec718)) - Max Batleforc, Claude Opus 5.5
- cut complexity and duplication, resolve the shortcuts - ([62d135c](https://github.com/batleforc/weebo-kura/commit/62d135c4639a49daf25f2520adc588177a12b065)) - Max Batleforc, Claude Opus 5.5
#### Miscellaneous Chores
- (**dev**) a persistent OpenBao for the local stack - ([5a01ea4](https://github.com/batleforc/weebo-kura/commit/5a01ea40e398492d25b616fade3f63e8f271d96d)) - Max Batleforc, Claude Opus 5.5
- (**dev**) tasks to try the plugin in Obsidian against the local stack - ([f2a0711](https://github.com/batleforc/weebo-kura/commit/f2a07116b15736d4ad4c14983550db71efadb597)) - Max Batleforc, Claude Opus 5.5
- (**version**) v0.1.0-rc.1 - ([a6cd96d](https://github.com/batleforc/weebo-kura/commit/a6cd96de91286acdf1d0bcca70d6e3390a44e452)) - Max Batleforc
- refresh the graphify graph - ([70ca299](https://github.com/batleforc/weebo-kura/commit/70ca299cfc5f3a805972d8e0c71b3380db1e141e)) - Max Batleforc, Claude Opus 5.5
- dated audit exceptions, and a suite vault apart from obsidian - ([fda4845](https://github.com/batleforc/weebo-kura/commit/fda4845bc08d478910dbaa5d3c9082d8036d9af2)) - Max Batleforc, Claude Opus 5.5

- - -

## [v0.1.0-rc.1](https://github.com/batleforc/weebo-kura/compare/bc8e20a8d219f4411cfa01b7de8cccd55457fdd4..v0.1.0-rc.1) - 2026-11-21
#### Features
- (**cli**) add kura verify and the pre-push migration check (RFC 0001 phase 9) - ([7ed3667](https://github.com/batleforc/weebo-kura/commit/7ed3667adbdaf454380c5632cb5b40ac89d12449)) - Max Batleforc, Claude Opus 5.5
- (**dev**) local stack with OpenBao, Authentik and PostgreSQL - ([2fc19eb](https://github.com/batleforc/weebo-kura/commit/2fc19eb0b5727dad24ab8eb537a9ecd2061e45c3)) - Max Batleforc, Claude Opus 5.5
- (**perf**) measure chunking and compression on a copy of a vault - ([3343ff9](https://github.com/batleforc/weebo-kura/commit/3343ff9ae559160ed23f42fd7d2e6d4c28ec3148)) - Max Batleforc, Claude Opus 5.5
- (**ui**) Mermaid diagrams in notes, and images up to 8 MiB - ([703a937](https://github.com/batleforc/weebo-kura/commit/703a937db5a899a4a5b0de85ae933e8d83522493)) - Max Batleforc, Claude Opus 5.5
- (**ui**) a folder with a note of its name opens it - ([833a813](https://github.com/batleforc/weebo-kura/commit/833a8130eb2b49a101a1f5803632bc8b8ff7828c)) - Max Batleforc, Claude Opus 5.5
- (**ui**) the folder tree's width, set with a handle beside it - ([bc54f05](https://github.com/batleforc/weebo-kura/commit/bc54f05780a405280e0a39823f70d4fde0339d60)) - Max Batleforc, Claude Opus 5.5
- (**ui**) the reader's history as a drawer, search in the tree, and a bilingual plugin - ([51ff2d3](https://github.com/batleforc/weebo-kura/commit/51ff2d3e1b0ae3538aee0625c719fc0b656d0dc3)) - Max Batleforc, Claude Opus 5.5
- (**ui**) a vault-centred console, its help, and the Obsidian plugin from it - ([35ac227](https://github.com/batleforc/weebo-kura/commit/35ac2275e813a1c100fc9fe6822aaffd6783c66c)) - Max Batleforc, Claude Opus 5.5
- (**ui**) notification rules and deliveries in the console (RFC 0003 phase 4) - ([a7e8e67](https://github.com/batleforc/weebo-kura/commit/a7e8e678e1ae166578a0198bf33f70322e5661e4)) - Max Batleforc, Claude Opus 5.5
- (**ui**) the back office of the web console (RFC 0003 phase 2) - ([6383e66](https://github.com/batleforc/weebo-kura/commit/6383e66106ac53c3dda4013b66b09c49fd9dcb98)) - Max Batleforc, Claude Opus 5.5
- the synced Obsidian configuration in the console (RFC 0008) - ([80cf466](https://github.com/batleforc/weebo-kura/commit/80cf4666113261ea3b04939da6e4c129686fbe70)) - Max Batleforc, Claude Opus 5.5
- folders sync as entries, empty ones included (RFC 0007) - ([e081be6](https://github.com/batleforc/weebo-kura/commit/e081be643aa104fd44d3b2c5f94c566bfc18fcb4)) - Max Batleforc, Claude Opus 5.5
- name device owners, and prove RFC 0006 in the browser - ([69682bb](https://github.com/batleforc/weebo-kura/commit/69682bbbd15027b5ddcf5055947bf09e01b1f2de)) - Max Batleforc, Claude Opus 5.5
- the last run's counts in the console, and RFC 0006 proven on the stack - ([1cd2cd7](https://github.com/batleforc/weebo-kura/commit/1cd2cd7351f42d80011787d1fa142f032c21ac01)) - Max Batleforc, Claude Opus 5.5
- where each device stands on a vault (RFC 0006) - ([807ead9](https://github.com/batleforc/weebo-kura/commit/807ead9162a696410ec9753cc42a02ee381d6deb)) - Max Batleforc, Claude Opus 5.5
- close the landing gaps of RFC 0003 and 0005, both Implemented - ([32d0bc0](https://github.com/batleforc/weebo-kura/commit/32d0bc0c2e1d31017d4cfc0fd91e5b098a66416d)) - Max Batleforc, Claude Opus 5.5
- batched uploads (RFC 0002 phase 3) - ([34fc574](https://github.com/batleforc/weebo-kura/commit/34fc5741119c14ac71e089906cf84ecd052fa88f)) - Max Batleforc, Claude Opus 5.5
- close the remaining gaps of RFC 0003, 0004 and 0005 - ([49e1b7d](https://github.com/batleforc/weebo-kura/commit/49e1b7d70bd34e62097f03832d87b88e6fc97881)) - Max Batleforc, Claude Opus 5.5
- webhook notifications, server side (RFC 0003 phase 3) - ([62cdb46](https://github.com/batleforc/weebo-kura/commit/62cdb46de27dac26d4aab5f6b511a3c8d957af11)) - Max Batleforc, Claude Opus 5.5
- folder-restricted viewers and the web reader (RFC 0005) - ([c2449ce](https://github.com/batleforc/weebo-kura/commit/c2449ce6df4eed142cff29c9559197b1bf17a63f)) - Max Batleforc, Claude Opus 5.5
- folder-scoped write tokens (RFC 0004) and the web console, phase 1 (RFC 0003) - ([b1854d5](https://github.com/batleforc/weebo-kura/commit/b1854d5f7034b596ac214223985d2d7ea8a7f30a)) - Max Batleforc, Claude Opus 5.5
- Mise en place de nouvelle RFC pour les évolution - ([a4d3ddd](https://github.com/batleforc/weebo-kura/commit/a4d3ddd6ed03d6747764df3d816d49c152c1d184)) - Max Batleforc
- batched chunk and manifest downloads (RFC 0002 phases 1 and 2) - ([1647256](https://github.com/batleforc/weebo-kura/commit/1647256ce6e6c2f5f1bc1376ce3412968bf86705)) - Max Batleforc, Claude Opus 5.5
- implement RFC 0001 phase 8 Obsidian configuration sync - ([ee747b1](https://github.com/batleforc/weebo-kura/commit/ee747b15d41dd8e904bb96b1d28b61b3f421fe9a)) - Max Batleforc, Claude Opus 5.5
- implement RFC 0001 phase 7 scoped tokens and opt-in search - ([1dd8b34](https://github.com/batleforc/weebo-kura/commit/1dd8b34061bf12d3eb2ba5ccb6b6a17361670bb9)) - Max Batleforc, Claude Opus 5.5
- implement RFC 0001 phase 6 operations, load tests and fixes - ([e95719d](https://github.com/batleforc/weebo-kura/commit/e95719dd1fab41c5ef675a4a3fe706dff676a288)) - Max Batleforc, Claude Opus 5.5
- implement RFC 0001 phase 5 history, retention and full resync - ([f19567c](https://github.com/batleforc/weebo-kura/commit/f19567cd0bb07cc46ce55008309b098770768594)) - Max Batleforc, Claude Opus 5.5
- implement RFC 0001 phase 4 merge, notifications and plugin - ([5af5497](https://github.com/batleforc/weebo-kura/commit/5af549745ab70c036ca4a6e05b6eba870b025abf)) - Max Batleforc, Claude Opus 5.5
- implement RFC 0001 phase 3 sync through kura-cli - ([0621b61](https://github.com/batleforc/weebo-kura/commit/0621b6120289bde8fece60cdadcf681216b0dd94)) - Max Batleforc, Claude Opus 5.5
- implement RFC 0001 phase 1 and phase 2 storage - ([b8e3c31](https://github.com/batleforc/weebo-kura/commit/b8e3c311fb4da8cb99bbe57f01f8bf370fb001d2)) - Max Batleforc, Claude Opus 5.5
- init project - ([64c35ef](https://github.com/batleforc/weebo-kura/commit/64c35efe04b5c04312ec0fdf1ab44a2c32af68b0)) - Max Batleforc
#### Bug Fixes
- (**auth**) follow a session through refreshed tokens by auth_time - ([89b2822](https://github.com/batleforc/weebo-kura/commit/89b28228146ad7663254415b517408cec179c44b)) - Max Batleforc, Claude Opus 5.5
- (**events**) re-check the vault role on open sockets - ([85e4fe9](https://github.com/batleforc/weebo-kura/commit/85e4fe9251ace3d0fa1865313b33b3720269e2c2)) - Max Batleforc, Claude Opus 5.5
- (**plugin**) turn a synced plugin off under every id it runs as - ([e1cd3fd](https://github.com/batleforc/weebo-kura/commit/e1cd3fd5e7f559247295d0e4b8bd45930cec12b5)) - Max Batleforc, Claude Opus 5.5
- (**plugin**) no epoch=undefined when the core has no epoch yet - ([36275df](https://github.com/batleforc/weebo-kura/commit/36275df56659f2f29b7a535260cf7fc0224b2b47)) - Max Batleforc, Claude Opus 5.5
- (**plugin**) keep the session across a vault switch, and offer sign-in again - ([15f99e0](https://github.com/batleforc/weebo-kura/commit/15f99e078665df85db24c8f53d9a3767dae92ebe)) - Max Batleforc, Claude Opus 5.5
- (**ui**) the critique's leftovers: status in words, ink for selection, no band - ([47000ba](https://github.com/batleforc/weebo-kura/commit/47000ba87c7e445e59d97992ef8d99fc9259a51c)) - Max Batleforc, Claude Opus 5.5
- (**ui**) the critique of 2026-11-18, from 26/40: names, phones, roles, words - ([d90ea51](https://github.com/batleforc/weebo-kura/commit/d90ea5108ad3f044f2e21b08c7d4db87760ae87d)) - Max Batleforc, Claude Opus 5.5
- resolve the findings of the 2026-11-21 project review - ([7225516](https://github.com/batleforc/weebo-kura/commit/72255166c9a33e3e91a93a01c0fd69f57888d714)) - Max Batleforc, Claude Opus 5.5
- close the 2026-11-15 global review - ([d1bad91](https://github.com/batleforc/weebo-kura/commit/d1bad9198b2295c25d1ab91d4500a535bb660e62)) - Max Batleforc, Claude Opus 5.5
- nine more findings of the 2026-11-15 global review - ([eb6ee24](https://github.com/batleforc/weebo-kura/commit/eb6ee2445459b1611168e251ec7f42eda9733d90)) - Max Batleforc, Claude Opus 5.5
- four findings of the 2026-11-15 global review - ([649579a](https://github.com/batleforc/weebo-kura/commit/649579aae083f62a2b4ab0c290353d9452c562d4)) - Max Batleforc, Claude Opus 5.5
#### Performance Improvements
- (**server**) list vaults in one transaction per tenant and fetch keys unlocked - ([fabe6d1](https://github.com/batleforc/weebo-kura/commit/fabe6d1b7d750a77b6d16c8080faa215fa4bcf8d)) - Max Batleforc, Claude Opus 5.5
#### Documentation
- (**rfc**) PR 9.2 done, phase 2 closed - ([4f8fe94](https://github.com/batleforc/weebo-kura/commit/4f8fe9403d13e0dc99a8089643168341a3c0db73)) - Max Batleforc, Claude Opus 5.5
- (**rfc**) PR 2.6 measured, decisions 52 and 38 kept - ([a423ec6](https://github.com/batleforc/weebo-kura/commit/a423ec641b3073137c15e84dd58e1b24a4a93cf3)) - Max Batleforc, Claude Opus 5.5
- (**rfc**) land RFC 0002 and 0004, revise 0003 and 0005 against the tree - ([1c22fce](https://github.com/batleforc/weebo-kura/commit/1c22fcec4c5b1a264eb32eec95bfb539049c2588)) - Max Batleforc, Claude Opus 5.5
- (**rfc**) draft RFC 0002 batched chunk and manifest transfers - ([37bc868](https://github.com/batleforc/weebo-kura/commit/37bc868e8ada45ee89b93ff119109f50364e32a2)) - Max Batleforc, Claude Opus 5.5
- (**rfc**) accept RFC 0001 kura sync backend - ([a7eca77](https://github.com/batleforc/weebo-kura/commit/a7eca776de8b86916442ff8c1a13343f69bb0876)) - Max Batleforc, Claude Opus 5.5
- (**rfc**) review and settle RFC 0001 kura sync backend - ([1d37ae3](https://github.com/batleforc/weebo-kura/commit/1d37ae3d9f42d63836e889bcc0954eab63ab0f57)) - Max Batleforc, Claude Opus 5.5
- keep the console's design critique - ([8971e9b](https://github.com/batleforc/weebo-kura/commit/8971e9b84bbcfb726f44dd940166b168d1b2bcbd)) - Max Batleforc, Claude Opus 5.5
- add RFC index with planned RFCs - ([d2d3f89](https://github.com/batleforc/weebo-kura/commit/d2d3f89d6eee703679ec7b943d670858a2962d93)) - Max Batleforc, Claude Opus 5.5
#### Tests
- (**heavy**) RFC 0002 observation 3, and an Android emulator for the plugin - ([2583a18](https://github.com/batleforc/weebo-kura/commit/2583a18aebc94f130757d0fdc10270d76b02c044)) - Max Batleforc, Claude Opus 5.5
#### Continuous Integration
- (**release**) tag-triggered release of the image, chart, CLI and plugin - ([f6b031d](https://github.com/batleforc/weebo-kura/commit/f6b031d3ff7ca842bad69a7feafde607331be87f)) - Max Batleforc, Claude Opus 5.5
- (**security**) daily scanners, SECURITY.md, scan the rendered chart - ([59f5dc7](https://github.com/batleforc/weebo-kura/commit/59f5dc79802f7ef4fab04cc01068a7013f760389)) - Max Batleforc, Claude Opus 5.5
- give cog verify a git identity on the runner - ([9687e64](https://github.com/batleforc/weebo-kura/commit/9687e644f5815f7bb7a47c91af338327bb29e3d1)) - Max Batleforc, Claude Opus 5.5
- stop .gitleaksignore quoting the placeholder it ignores - ([9b90870](https://github.com/batleforc/weebo-kura/commit/9b908708fae8e303409babefb3344bcea5de1f0d)) - Max Batleforc, Claude Opus 5.5
- allow a run by hand - ([a76e6df](https://github.com/batleforc/weebo-kura/commit/a76e6df6a334099bc4f9277458d13e26e199c199)) - Max Batleforc, Claude Opus 5.5
- unblock the checks: a mock token in RFC 0004, picomatch in Trivy - ([ac68e90](https://github.com/batleforc/weebo-kura/commit/ac68e90293ef38fc0aea8fa9aec1def7935377c8)) - Max Batleforc, Claude Opus 5.5
#### Refactoring
- (**db**) squash the migrations into 0001_init - ([f53eca6](https://github.com/batleforc/weebo-kura/commit/f53eca6569b15fa1bfba5adba478300a2a5ec718)) - Max Batleforc, Claude Opus 5.5
- cut complexity and duplication, resolve the shortcuts - ([62d135c](https://github.com/batleforc/weebo-kura/commit/62d135c4639a49daf25f2520adc588177a12b065)) - Max Batleforc, Claude Opus 5.5
#### Miscellaneous Chores
- (**dev**) a persistent OpenBao for the local stack - ([5a01ea4](https://github.com/batleforc/weebo-kura/commit/5a01ea40e398492d25b616fade3f63e8f271d96d)) - Max Batleforc, Claude Opus 5.5
- (**dev**) tasks to try the plugin in Obsidian against the local stack - ([f2a0711](https://github.com/batleforc/weebo-kura/commit/f2a07116b15736d4ad4c14983550db71efadb597)) - Max Batleforc, Claude Opus 5.5
- refresh the graphify graph - ([70ca299](https://github.com/batleforc/weebo-kura/commit/70ca299cfc5f3a805972d8e0c71b3380db1e141e)) - Max Batleforc, Claude Opus 5.5
- dated audit exceptions, and a suite vault apart from obsidian - ([fda4845](https://github.com/batleforc/weebo-kura/commit/fda4845bc08d478910dbaa5d3c9082d8036d9af2)) - Max Batleforc, Claude Opus 5.5

- - -

Changelog generated by [cocogitto](https://github.com/cocogitto/cocogitto).
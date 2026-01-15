# Changelog

## [1.1.7](https://github.com/hhanh00/zcash-walletd/compare/zcash-walletd-v1.1.6...zcash-walletd-v1.1.7) (2026-01-15)


### Bug Fixes

* cleanup stale data at startup ([#56](https://github.com/hhanh00/zcash-walletd/issues/56)) ([3aad080](https://github.com/hhanh00/zcash-walletd/commit/3aad080806216c9effc2d86fcecaf584ec9045a0))

## [1.1.6](https://github.com/hhanh00/zcash-walletd/compare/zcash-walletd-v1.1.5...zcash-walletd-v1.1.6) (2026-01-08)


### Bug Fixes

* notify_tx called before db_tx.commit ([#55](https://github.com/hhanh00/zcash-walletd/issues/55)) ([6c11fb1](https://github.com/hhanh00/zcash-walletd/commit/6c11fb19f7c999c4f421b91a44fdd2783fed669e))
* return error for unknown txid ([#51](https://github.com/hhanh00/zcash-walletd/issues/51)) ([617136c](https://github.com/hhanh00/zcash-walletd/commit/617136c72f30c5a9b33dec72ff13826a2af4cda8))

## [1.1.5](https://github.com/hhanh00/zcash-walletd/compare/zcash-walletd-v1.1.4...zcash-walletd-v1.1.5) (2025-11-28)


### Bug Fixes

* support for NU6.1 ([#49](https://github.com/hhanh00/zcash-walletd/issues/49)) ([59d9b83](https://github.com/hhanh00/zcash-walletd/commit/59d9b83dde8724ff58f6ca8d5eed220ed80ed06c))

## [1.1.4](https://github.com/hhanh00/zcash-walletd/compare/zcash-walletd-v1.1.3...zcash-walletd-v1.1.4) (2025-10-28)


### Bug Fixes

* race condition possibility in account/address creation ([#46](https://github.com/hhanh00/zcash-walletd/issues/46)) ([dc84da2](https://github.com/hhanh00/zcash-walletd/commit/dc84da2b6299243469125c0075f712e9347ca08d))

## [1.1.3](https://github.com/hhanh00/zcash-walletd/compare/zcash-walletd-v1.1.2...zcash-walletd-v1.1.3) (2025-08-29)


### Bug Fixes

* release new docker image ([#43](https://github.com/hhanh00/zcash-walletd/issues/43)) ([abf7551](https://github.com/hhanh00/zcash-walletd/commit/abf7551f05afd3d117880096b140865d7adf4a73))

## [1.1.2](https://github.com/hhanh00/zcash-walletd/compare/zcash-walletd-v1.1.1...zcash-walletd-v1.1.2) (2025-08-29)


### Bug Fixes

* return original UA in transfers ([#39](https://github.com/hhanh00/zcash-walletd/issues/39)) ([975cc84](https://github.com/hhanh00/zcash-walletd/commit/975cc84c6b59bb3022e622a966b905e278779692))
* update docker image build script to latest version ([#37](https://github.com/hhanh00/zcash-walletd/issues/37)) ([86dc563](https://github.com/hhanh00/zcash-walletd/commit/86dc5638f04e9ecd3f56a891e7b73f7cadfdb54d))

## [1.1.1](https://github.com/hhanh00/zcash-walletd/compare/zcash-walletd-v1.1.0...zcash-walletd-v1.1.1) (2025-08-29)


### Bug Fixes

* config ([#34](https://github.com/hhanh00/zcash-walletd/issues/34)) ([1edde7c](https://github.com/hhanh00/zcash-walletd/commit/1edde7c54f09883558b103a53eed6ac442cd3ed4))
* missing call to tx_notify ([#35](https://github.com/hhanh00/zcash-walletd/issues/35)) ([aa1580a](https://github.com/hhanh00/zcash-walletd/commit/aa1580a9b280ee001c6ed0ac1dc70a367d39c3f2))
* retrieve hash at birth height ([#32](https://github.com/hhanh00/zcash-walletd/issues/32)) ([8579df7](https://github.com/hhanh00/zcash-walletd/commit/8579df7835e7d2567dd12d0c0f41380ce8e2d75d))

## [1.1.0](https://github.com/hhanh00/zcash-walletd/compare/zcash-walletd-v1.0.0...zcash-walletd-v1.1.0) (2025-08-29)


### Features

* [sync] detect spends ([#23](https://github.com/hhanh00/zcash-walletd/issues/23)) ([3a21b85](https://github.com/hhanh00/zcash-walletd/commit/3a21b852bd4feb73020f7db992eb685cc00594c4))
* [sync] scan blocks ([04227b5](https://github.com/hhanh00/zcash-walletd/commit/04227b56fa29c04347436cf925da2b7b71882e4c))
* [sync] scan transaction ([#22](https://github.com/hhanh00/zcash-walletd/issues/22)) ([00302b3](https://github.com/hhanh00/zcash-walletd/commit/00302b3f098ed87de9ecd0add2daa8305e449ac7))
* [sync] store sync data ([#24](https://github.com/hhanh00/zcash-walletd/issues/24)) ([d97c7b5](https://github.com/hhanh00/zcash-walletd/commit/d97c7b5844cc2ad8d6f930d48a63dad52436678c))
* add release please ([cd47859](https://github.com/hhanh00/zcash-walletd/commit/cd4785925b7bd71f73f7769dce356866ed2187f9))
* ci ([8a89886](https://github.com/hhanh00/zcash-walletd/commit/8a89886b73112823bc1ae980922910b46be56032))
* install lightwalletd in CI ([#6](https://github.com/hhanh00/zcash-walletd/issues/6)) ([a1ff880](https://github.com/hhanh00/zcash-walletd/commit/a1ff880c344bc4c519c16b04d6f7d7a678c00b10))
* regtest setting in config ([#5](https://github.com/hhanh00/zcash-walletd/issues/5)) ([e4aa910](https://github.com/hhanh00/zcash-walletd/commit/e4aa910c5a66c6c15b77ac1b8411e289871f8ce8))
* setup regtest ([#4](https://github.com/hhanh00/zcash-walletd/issues/4)) ([ee8a1af](https://github.com/hhanh00/zcash-walletd/commit/ee8a1af535393757d5ec57a71f5bbad96af7a055))
* update to NU-6 and librustzcash dependencies ([#1](https://github.com/hhanh00/zcash-walletd/issues/1)) ([ee97b92](https://github.com/hhanh00/zcash-walletd/commit/ee97b92be37ba9c7768876a71c09eef66a8c6de3))


### Bug Fixes

* [sync] previous commmit ([#21](https://github.com/hhanh00/zcash-walletd/issues/21)) ([e3bfe28](https://github.com/hhanh00/zcash-walletd/commit/e3bfe2899fbdb2a84e1f3676e4ece50043d296bf))
* change from vk to ufvk ([#25](https://github.com/hhanh00/zcash-walletd/issues/25)) ([5b8e9db](https://github.com/hhanh00/zcash-walletd/commit/5b8e9db0b66a66ab488f8e5607b118fecacb345b))
* fix clippy warnings ([#3](https://github.com/hhanh00/zcash-walletd/issues/3)) ([05ef06a](https://github.com/hhanh00/zcash-walletd/commit/05ef06ae5448a91e928660edf032b85386d23110))
* invalid tx amount when tx spends ([#29](https://github.com/hhanh00/zcash-walletd/issues/29)) ([c583717](https://github.com/hhanh00/zcash-walletd/commit/c583717c3c31af5ebe0b11d420cad91440b7c9a9))
* should panic if db schema is old ([#28](https://github.com/hhanh00/zcash-walletd/issues/28)) ([1711829](https://github.com/hhanh00/zcash-walletd/commit/1711829c3e7cb8d9850f2f9d1c5be9795237b0ba))
* support ua with multiple receivers ([#27](https://github.com/hhanh00/zcash-walletd/issues/27)) ([2f09caa](https://github.com/hhanh00/zcash-walletd/commit/2f09caa2d05118d7c2a38872002f3fca5d512319))
* wire up unified scanner ([#26](https://github.com/hhanh00/zcash-walletd/issues/26)) ([1f8a545](https://github.com/hhanh00/zcash-walletd/commit/1f8a545e835aff70e17f308a98d0719dbddde676))
* zcashd deprecation option in zcash.conf ([#14](https://github.com/hhanh00/zcash-walletd/issues/14)) ([02e0e66](https://github.com/hhanh00/zcash-walletd/commit/02e0e66580e463c354b73e986ba2d57ef6b4baeb))

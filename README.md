`zcash-walletd` is a shielded only (sapling) REST wallet
for zcash.

Primary intended to work with the BTCPayServer payment gateway,
it offers a REST interface that can be useful in other scenarios.

For instance, it implements a subset of the `monero-wallet` API,
which allows it to be used interchangeably.

## Features

- `zcash-walletd` maps accounts and subaccounts to diversified addresses,
- Millions of accounts and sub accounts are supported without significant performance loss,
- can be used to monitor the zcash blockchain for incoming transactions
- can call an external URL on new transactions
- Cold wallet - `zcash-walletd` does not use seeds or secret keys

## Build

```
# cargo build --release
```

## Configuration

- `zcash-walletd` looks for an environment variable `VK` that must contains the viewing key of the wallet
- Optionally, if a `BIRTH_HEIGHT` variable is present it will indicate the starting scan height
- `BIRTH_HEIGHT` is only used for the initial sync

## Command line args

- Passing `--rescan` will instruct `zcash-walletd` to resync from the birth height or the sapling activation
height

## Docker

To build a docker image: Run from the project directory

```
# ./docker/build.sh
```

The latest image is available on DockerHub under `hhanh00/zcash-walletd:latest`

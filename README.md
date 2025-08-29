`zcash-walletd` is a shielded only (sapling) REST wallet
for zcash.

Primary intended to work with the BTCPayServer payment gateway,
it offers a REST interface that can be useful in other scenarios.

For instance, it implements a subset of the `monero-wallet` API,
which allows it to be used interchangeably in some cases.

## Features

### Account Management

Create diversified addresses on demand and map them to account #
and sub account #. BTCPay associates each store to an account and
each invoice into a sub account.

Millions of accounts and sub accounts are supported without significant performance loss.

### Monitor the Blockchain and detect incoming payments

When a customer pays an invoice, `zcash-walletd` sees the received
notes and makes a REST/POST request to notify the payment gateway.

Partial payments are supported.

### Security

Wallet is view only and does not contain the main account seed or secret key.

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

## Orchard
Support for Orchard and UA was added in 1.1.2. You MUST delete the database file
because the previous schema is not compatible.

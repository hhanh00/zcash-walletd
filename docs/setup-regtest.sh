#!/bin/sh
set -x
zcashd --datadir=regtest --daemon
sleep 10
zcash-cli --datadir=regtest generate 150
zcash-cli --datadir=regtest z_getnewaccount
zcash-cli --datadir=regtest z_getaddressforaccount 0
UA=`zcash-cli --datadir=regtest listaddresses | jq -r '.[0].unified[0].addresses[0].address'`
zcash-cli --datadir=regtest z_shieldcoinbase '*' $UA
sleep 5
zcash-cli --datadir=regtest z_getoperationresult
zcash-cli --datadir=regtest generate 10
sleep 1
zcash-cli --datadir=regtest z_sendmany $UA '[{"address": "tmGys6dBuEGjch5LFnhdo5gpSa7jiNRWse6", "amount": 5.40}]' 1 null 'AllowRevealedRecipients'
sleep 5
zcash-cli --datadir=regtest z_getoperationresult
zcash-cli --datadir=regtest generate 10
sleep 1
zcash-cli --datadir=regtest z_sendmany $UA '[{"address": "tmP9jLgTnhDdKdWJCm4BT2t6acGnxqP14yU", "amount": 1.20}]' 1 null 'AllowRevealedRecipients'
sleep 5
zcash-cli --datadir=regtest z_getoperationresult
zcash-cli --datadir=regtest generate 10

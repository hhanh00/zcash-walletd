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
zcash-cli --datadir=regtest z_sendmany $UA '[{"address": "uregtest1xrgsdqnrf69lspz6etq6mlnm7pkpkuaulr30u0avrffr56dyfktprw309vxs8q3tlm0yq9x8kvm7348et08xnwqg2swgd2482rkg0l0jywkrhvldesvwhw3wxgcnjeem4vstv0x4je2ypk5q4fwnhzlaefjnpscunzrfd03eqgz5cf0v", "amount": 5.40}]' 1 null 'AllowRevealedRecipients'
sleep 5
zcash-cli --datadir=regtest z_getoperationresult
zcash-cli --datadir=regtest generate 10
sleep 1
zcash-cli --datadir=regtest z_sendmany $UA '[{"address": "zregtestsapling1qag0mpkwcratr9zweyk973dzukaln3svpl0v8fpydajq8aq8ghsq0ah3my0qc2admygg6xt4snh", "amount": 1.20}]' 1 null 'AllowRevealedRecipients'
sleep 5
zcash-cli --datadir=regtest z_getoperationresult
zcash-cli --datadir=regtest generate 10

# seed phrase for test account
# tobacco symbol exchange token often pet call crew unique bachelor purpose police coil world lumber sleep bottom monkey catch giraffe peanut name cigar private

# addresses
# uregtest1xrgsdqnrf69lspz6etq6mlnm7pkpkuaulr30u0avrffr56dyfktprw309vxs8q3tlm0yq9x8kvm7348et08xnwqg2swgd2482rkg0l0jywkrhvldesvwhw3wxgcnjeem4vstv0x4je2ypk5q4fwnhzlaefjnpscunzrfd03eqgz5cf0v
# zregtestsapling1qag0mpkwcratr9zweyk973dzukaln3svpl0v8fpydajq8aq8ghsq0ah3my0qc2admygg6xt4snh
# tmLC3vYhCwDQu1RatZwyxfG9ejTLVUDnDso

# vk
# uviewregtest10lkfv9ck80w7hc50x02fkzwl004glax8gtyg6n3edgy5ld34xvutln5zwlezpmtadv9v2jge0damef7egg8tk93xncq73k0fdzpfrecpzmres8ucz82m8h9ephp53vasten7xrf95h9egdhyg2fqu2qz3hgyy0k6tny6d28m5duuzk72ma0nfr2y5cxqwjscspsdm5qkaafc9edtpzapmfxgzcdkqr60atx32g6q8fxhhh9n0hueslvzy04xyx5353nmmxx2k7uxwdv6t9y626f0d03lgufgkct3gkyxp4u24xdz9l5jsa5ne8cw9s5cjqernqj7xqmwzuc7lad6c7ayqk2ry3e66qea5pmq32a9v4spfswmtsvklljmd0fc4pk8f32g7snzxyrlmnkguch3execr9kqx02a6dc2ryuzvrg8vrrfxjkve6tpyk4vfz9j2zkuws9g5e06wm744yzsye3w74qwjrn5t2rzqfn6zmr8fgkjea8c

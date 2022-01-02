#!/bin/bash

# Call this script from root directory!

docker build -t zcash-walletd -f docker/Dockerfile .

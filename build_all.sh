#!/bin/bash
set -e

cd ./defi-borrow
./build.sh

cd ../draw
./build.sh

cd ../pool
./build.sh
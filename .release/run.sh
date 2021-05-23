#!/bin/bash

cd card-playroom-server && \
diesel migration run && \
./release/card-playroom-server
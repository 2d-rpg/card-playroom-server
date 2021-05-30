#!/bin/bash

# For Ubuntu

# Update package index
sudo apt-get update
# Install requires
sudo apt-get install -y \
  apt-transport-https \
  ca-certificates \
  curl \
  software-properties-common
# Install GPG public key
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo apt-key add -

sudo apt-key fingerprint 0EBFCD88
# apt repository Setting
sudo add-apt-repository \
   "deb [arch=amd64] https://download.docker.com/linux/ubuntu \
   $(lsb_release -cs) \
   stable"
# Install docker-ce
sudo apt-get update
sudo apt-get install -y docker-ce
docker version
# Install Docker compose
sudo curl -L "https://github.com/docker/compose/releases/download/1.26.0/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
sudo chmod +x /usr/local/bin/docker-compose
docker-compose --version

# Git clone server repo
git clone https://github.com/2d-rpg/card-playroom-server.git && \
cd card-playroom-server/.release

# Docker pull & docker-compose up
sudo docker pull natadecocoa/card-playroom-server:latest && \
sudo docker-compose up

# Enter web bash
sudo docker-compose exec web ./run.sh
